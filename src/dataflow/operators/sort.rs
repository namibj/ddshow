use std::panic::Location;

use crate::dataflow::operators::Multiply;
use differential_dataflow::{
    difference::Abelian, lattice::Lattice, operators::Reduce, AsCollection, Collection, Data,
    ExchangeData, Hashable,
};
use timely::dataflow::{operators::Map, Scope};

#[allow(dead_code)]
const DEFAULT_HIERARCHICAL_BUCKETS: [u64; 16] =
    [60, 56, 52, 48, 44, 40, 36, 32, 28, 24, 20, 16, 12, 8, 4, 0];

pub trait SortBy<T> {
    type Output;

    #[track_caller]
    fn sort_by<F, K>(&self, key: F) -> Self::Output
    where
        F: Fn(&T) -> K + Clone + 'static,
        K: Ord,
    {
        let caller = Location::caller();
        self.sort_by_named(
            &format!(
                "SortBy @ {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            ),
            key,
        )
    }

    fn sort_by_named<F, K>(&self, name: &str, key: F) -> Self::Output
    where
        F: Fn(&T) -> K + Clone + 'static,
        K: Ord,
    {
        self.hierarchical_sort_core(name, [0].iter().copied(), key)
    }

    #[track_caller]
    fn hierarchical_sort_by<F, K>(&self, key: F) -> Self::Output
    where
        F: Fn(&T) -> K + Clone + 'static,
        K: Ord,
    {
        let caller = Location::caller();
        self.hierarchical_sort_by_named(
            &format!(
                "HierarchicalSortBy @ {}:{}:{}",
                caller.file(),
                caller.line(),
                caller.column()
            ),
            key,
        )
    }

    fn hierarchical_sort_by_named<F, K>(&self, name: &str, key: F) -> Self::Output
    where
        F: Fn(&T) -> K + Clone + 'static,
        K: Ord,
    {
        self.hierarchical_sort_core(name, DEFAULT_HIERARCHICAL_BUCKETS.iter().copied(), key)
    }

    fn hierarchical_sort_core<B, F, K>(&self, name: &str, buckets: B, key: F) -> Self::Output
    where
        B: IntoIterator<Item = u64>,
        F: Fn(&T) -> K + Clone + 'static,
        K: Ord;
}

impl<S, K, D, R> SortBy<D> for Collection<S, (K, D), R>
where
    S: Scope,
    S::Timestamp: Lattice,
    K: ExchangeData,
    D: ExchangeData + Hashable<Output = u64> + Default,
    Vec<D>: ExchangeData,
    (K, D): Hashable,
    (K, Vec<D>): Hashable,
    ((u64, K), Vec<D>): ExchangeData,
    (u64, K): ExchangeData + Hashable,
    R: Abelian + ExchangeData + Multiply<Output = R> + Into<isize> + From<i8>,
{
    type Output = Collection<S, (K, Vec<D>), R>;

    fn hierarchical_sort_core<B, F, DK>(&self, name: &str, buckets: B, key: F) -> Self::Output
    where
        B: IntoIterator<Item = u64>,
        F: Fn(&D) -> DK + Clone + 'static,
        DK: Ord,
    {
        self.scope().region_named(name, |region| {
            let this = self.enter_region(region);

            // Utilizes hierarchical aggregation to minimize the number of recomputation that must happen
            let mut hashed =
                this.map(|(key, data)| ((data.hashed(), key), vec![(data, R::from(1))]));
            for bucket in buckets {
                hashed = build_sort_bucket(hashed, key.clone(), 1u64 << bucket);
            }

            hashed
                .inner
                .map(|(((_hash, key), data), time, diff)| {
                    let data = data
                        .into_iter()
                        .flat_map(|(data, inner_diff)| {
                            (0..inner_diff.into()).map(move |_| data.clone())
                        })
                        .collect::<Vec<_>>();

                    ((key, data), time, diff)
                })
                .as_collection()
                .leave_region()
        })
    }
}

// TODO: Switch to `TinyVec<[(D, R); 16]>`
type Bucketed<S, K, D, R> = Collection<S, ((u64, K), Vec<(D, R)>), R>;

fn build_sort_bucket<S, K, D, R, F, DK>(
    hashed: Bucketed<S, K, D, R>,
    key: F,
    bucket: u64,
) -> Bucketed<S, K, D, R>
where
    S: Scope,
    S::Timestamp: Lattice,
    D: Data + Default,
    Vec<(D, R)>: ExchangeData,
    ((u64, K), Vec<D>): ExchangeData,
    (u64, K): ExchangeData + Hashable,
    R: Abelian + ExchangeData + Multiply<Output = R> + From<i8>,
    F: Fn(&D) -> DK + 'static,
    DK: Ord,
{
    let input = hashed.map(move |((hash, key), data)| ((hash % bucket, key), data));

    // TODO: The buckets could take advantage of their inputs already being sorted
    //       by using k-way merges https://en.wikipedia.org/wiki/K-way_merge_algorithm
    //       See also https://docs.rs/itertools/0.10.0/src/itertools/kmerge_impl.rs.html
    input.reduce_named::<_, Vec<(D, R)>, R>("SortByBucket", move |_key, input, output| {
        let mut data = Vec::with_capacity(input.iter().map(|(data, _)| data.len()).sum());
        data.extend(input.iter().flat_map(|(data, diff)| {
            data.iter().cloned().map(move |(data, inner_diff)| {
                (data, {
                    #[cfg(not(feature = "timely-next"))]
                    let result = diff.clone() * inner_diff;
                    #[cfg(feature = "timely-next")]
                    let result = diff.clone().multiply(&inner_diff);
                    result
                })
            })
        }));

        data.sort_unstable_by_key(|(data, _diff)| key(data));

        let mut idx = 0;
        while idx + 1 < data.len() {
            if data[idx].1.is_zero() {
                data.remove(idx);
            } else if data[idx].0 == data[idx + 1].0 {
                let diff = data[idx + 1].1.clone();
                #[cfg(not(feature = "timely-next"))]
                {
                    data[idx].1 += &diff;
                };
                #[cfg(feature = "timely-next")]
                {
                    differential_dataflow::difference::Semigroup::plus_equals(
                        &mut data[idx].1,
                        &diff,
                    );
                };
                data.remove(idx + 1);
            } else {
                idx += 1;
            }
        }

        output.push((data, R::from(1)));
    })
}

#[cfg(test)]
mod tests {
    use crate::dataflow::operators::{CrossbeamExtractor, CrossbeamPusher, SortBy};
    use differential_dataflow::input::Input;
    use rand::Rng;
    use timely::dataflow::operators::Capture;

    #[test]
    fn ensure_sorting() {
        let (send, recv) = crossbeam_channel::unbounded();
        timely::execute_directly(|worker| {
            let (mut input, probe) = worker.dataflow(|scope| {
                let (input, collection) = scope.new_collection();

                let sorted = collection
                    .hierarchical_sort_by(|&int| int)
                    .map(|((), sorted)| sorted);
                sorted.inner.capture_into(CrossbeamPusher::new(send));

                (input, sorted.probe())
            });

            let mut rng = rand::thread_rng();
            for epoch in 0..rng.gen_range(10..=50) {
                input.advance_to(epoch);

                for _ in 0..rng.gen_range(100..=1000) {
                    let int = rng.gen::<isize>();
                    input.insert(((), int));

                    if rng.gen_bool(0.5) {
                        input.insert(((), int));
                    }

                    if rng.gen_bool(0.25) {
                        input.remove(((), int));
                    }
                }

                input.flush();
                worker.step_or_park_while(None, || probe.less_than(input.time()));
            }

            worker.step_or_park_while(None, || probe.less_than(input.time()));
        });

        let mut result = CrossbeamExtractor::new(recv).extract_all();
        assert_eq!(result.len(), 1);
        for window in result.remove(0).windows(2) {
            assert!(window[0] <= window[1]);
        }
    }

    #[test]
    fn ensure_retractions_propagate() {
        let (send, recv) = crossbeam_channel::unbounded();
        timely::execute_directly(|worker| {
            let (mut input, probe) = worker.dataflow(|scope| {
                let (input, collection) = scope.new_collection();

                let sorted = collection
                    .hierarchical_sort_by(|&int| int)
                    .map(|((), sorted)| sorted);
                sorted.inner.capture_into(CrossbeamPusher::new(send));

                (input, sorted.probe())
            });

            input.insert(((), 1));
            input.insert(((), 1));
            input.remove(((), 1));
            input.insert(((), 2));
            input.insert(((), 3));
            input.insert(((), 4));
            input.insert(((), 5));

            input.advance_to(1);
            input.flush();
            worker.step_or_park_while(None, || probe.less_than(input.time()));
        });

        let result = CrossbeamExtractor::new(recv).extract_all();
        assert_eq!(result, vec![vec![1, 2, 3, 4, 5]]);
    }

    #[test]
    fn ensure_retractions_propagate_across_timestamps() {
        let (send, recv) = crossbeam_channel::unbounded();
        timely::execute_directly(|worker| {
            let (mut input, probe) = worker.dataflow(|scope| {
                let (input, collection) = scope.new_collection();

                let sorted = collection
                    .hierarchical_sort_by(|&int| int)
                    .map(|((), sorted)| sorted);
                sorted.inner.capture_into(CrossbeamPusher::new(send));

                (input, sorted.probe())
            });

            input.insert(((), 1));
            input.insert(((), 1));
            input.insert(((), 2));
            input.insert(((), 3));
            input.insert(((), 4));
            input.insert(((), 5));
            input.advance_to(1);

            input.remove(((), 1));
            input.advance_to(2);

            input.insert(((), 2));
            input.advance_to(3);

            input.flush();
            worker.step_or_park_while(None, || probe.less_than(input.time()));
        });

        let result = CrossbeamExtractor::new(recv).extract_all();
        assert_eq!(result, vec![vec![1, 2, 2, 3, 4, 5]]);
    }

    #[test]
    fn fully_retract_values() {
        let (send, recv) = crossbeam_channel::unbounded();
        timely::execute_directly(|worker| {
            let (mut input, probe) = worker.dataflow(|scope| {
                let (input, collection) = scope.new_collection();

                let sorted = collection
                    .hierarchical_sort_by(|&int| int)
                    .map(|((), sorted)| sorted);
                sorted.inner.capture_into(CrossbeamPusher::new(send));

                (input, sorted.probe())
            });

            input.insert(((), 1));
            input.insert(((), 2));
            input.insert(((), 3));
            input.insert(((), 4));
            input.insert(((), 5));
            input.advance_to(1);

            input.remove(((), 1));
            input.remove(((), 2));
            input.remove(((), 3));
            input.remove(((), 4));
            input.remove(((), 5));
            input.advance_to(2);

            input.flush();
            worker.step_or_park_while(None, || probe.less_than(input.time()));
        });

        let result = CrossbeamExtractor::new(recv).extract_all();
        assert_eq!(result, Vec::<Vec<usize>>::new());
    }
}
