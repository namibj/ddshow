use crate::{
    args::Args,
    dataflow::{ChannelId, OperatorAddr, OperatorId, PortId, WorkerId, WorkerTimelineEvent},
};
use abomonation_derive::Abomonation;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::BufWriter,
    time::Duration,
};
use tera::{Context, Tera};

const GRAPH_HTML: &str = include_str!("graph.html");
const GRAPH_CSS: &str = include_str!("graph.css");
const GRAPH_JS: &str = include_str!("graph.js");
const D3_JS: &str = include_str!("d3.v5.js");
const DAGRE_JS: &str = include_str!("dagre-d3.js");
const ECHARTS_JS: &str = include_str!("echarts.min.js");

pub fn render(
    args: &Args,
    nodes: Vec<Node>,
    subgraphs: Vec<Subgraph>,
    edges: Vec<Edge>,
    palette_colors: Vec<String>,
    timeline_events: Vec<WorkerTimelineEvent>,
) -> Result<()> {
    let output_dir = &args.output_dir;
    tracing::info!(output_dir = ?output_dir, "writing graph files to disk");

    fs::create_dir_all(output_dir).context("failed to create output directory")?;

    fs::write(output_dir.join("graph.html"), GRAPH_HTML)
        .context("failed to write output graph to file")?;

    fs::write(output_dir.join("graph.css"), GRAPH_CSS)
        .context("failed to write output graph to file")?;

    fs::write(output_dir.join("d3.v5.js"), D3_JS)
        .context("failed to write output graph to file")?;

    fs::write(output_dir.join("dagre-d3.js"), DAGRE_JS)
        .context("failed to write output graph to file")?;

    fs::write(output_dir.join("echarts.min.js"), ECHARTS_JS)
        .context("failed to write output graph to file")?;

    let graph_data = GraphData {
        nodes,
        subgraphs,
        edges,
        palette_colors,
        timeline_events,
    };

    // TODO: This shouldn't be here
    if let Some(json_path) = args.dump_json.as_ref() {
        tracing::info!(json_path = ?json_path, "dumping json to file");

        let json_file = File::create(json_path)
            .with_context(|| format!("failed to create json file at {}", json_path.display()))?;

        serde_json::to_writer(BufWriter::new(json_file), &graph_data)
            .context("failed to write json data to file")?;
    }

    let context =
        Context::from_serialize(graph_data).context("failed to render graph context as json")?;

    let rendered_js =
        Tera::one_off(GRAPH_JS, &context, false).context("failed to render output graph")?;

    fs::write(output_dir.join("graph.js"), rendered_js)
        .context("failed to write output graph to file")?;

    Ok(())
}

//  - whether differential logging was enabled
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize)]
pub struct DDShowStats {
    pub program: ProgramStats,
    pub workers: BTreeMap<WorkerId, WorkerStats>,
    pub dataflows: BTreeMap<OperatorId, DataflowStats>,
    // TODO: Should/would this be better as a `BTree<(WorkerId, OperatorId), NodeStats>`?
    //       What about address-based lookups?
    pub nodes: Vec<NodeStats>,
    pub channels: Vec<ChannelStats>,
    pub differential_enabled: bool,
}

// - Program stats
//  - # workers
//  - # dataflows
//  - # nodes
//  - # operators
//  - # subgraphs
//  - # channels
//  - # events
//  - # missing nodes
//  - # missing edges
//  - total program runtime
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct ProgramStats {
    pub workers: usize,
    pub dataflows: usize,
    pub nodes: usize,
    pub operators: usize,
    pub subgraphs: usize,
    pub channels: usize,
    pub events: usize,
    pub runtime: Duration,
    // TODO: Missing nodes & edges
}

// - Worker stats
//   - total worker runtime
//  - # dataflows
//  - # nodes
//  - # operators
//  - # subgraphs
//  - # channels
//  - # events
//  - # missing nodes
//  - # missing edges
//  - list of dataflow addresses
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct WorkerStats {
    pub id: WorkerId,
    pub dataflows: usize,
    pub nodes: usize,
    pub operators: usize,
    pub subgraphs: usize,
    pub channels: usize,
    pub events: usize,
    pub runtime: Duration,
    pub dataflow_addrs: Vec<OperatorAddr>,
    // TODO: Missing nodes & edges
}

// - Dataflow stats
//   - creation time
//   - drop time
//   - # of contained operators
//   - # of contained subgraphs
//   - # of contained channels
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct DataflowStats {
    pub id: OperatorId,
    pub addr: OperatorAddr,
    pub worker: WorkerId,
    pub nodes: usize,
    pub operators: usize,
    pub subgraphs: usize,
    pub channels: usize,
    pub lifespan: Lifespan,
}

// - Nodes
//   - id
//   - worker
//   - address
//   - name
//   - inputs
//   - outputs
//   - whether it's a subgraph
//   - whether it's a root dataflow
//   - number of invocations
//   - max activation time
//   - min activation time
//   - average activation time
//   - all activation durations
//   - creation time
//   - drop time
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct NodeStats {
    pub id: OperatorId,
    pub addr: OperatorAddr,
    pub worker: WorkerId,
    pub name: String,
    pub inputs: Vec<PortId>,
    pub outputs: Vec<PortId>,
    pub lifespan: Lifespan,
    pub kind: NodeKind,
    pub activations: ActivationStats,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Abomonation,
)]
pub enum NodeKind {
    Operator,
    Subgraph,
    Dataflow,
}

impl Default for NodeKind {
    fn default() -> Self {
        Self::Operator
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct ActivationStats {
    pub activations: usize,
    pub max: Duration,
    pub min: Duration,
    pub average: Duration,
    pub data_points: Vec<Duration>,
    // TODO: Standard deviation, standard error
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Deserialize,
    Serialize,
    Abomonation,
)]
pub struct Lifespan {
    pub birth: Duration,
    pub death: Duration,
}

// - Edges
//   - id
//   - worker
//   - address
//   - name
//   - edge kind
//   - edge id (is this even a real thing?)
//   - source node
//   - dest node
//   - creation time
//   - drop time
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Deserialize, Serialize, Abomonation,
)]
pub struct ChannelStats {
    // TODO: Do these two actually even exist?
    pub id: ChannelId,
    // TODO: Make `ChannelAddr`
    pub addr: OperatorAddr,
    pub worker: WorkerId,
    pub source_node: OperatorId,
    pub dest_node: OperatorId,
    pub kind: ChannelKind,
    pub lifespan: Lifespan,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Abomonation,
)]
pub enum ChannelKind {
    Ingress,
    Egress,
    Normal,
}

impl Default for ChannelKind {
    fn default() -> Self {
        Self::Normal
    }
}

// - Arrangement stats
//   - operator address
//   - max arrangement size
//   - min arrangement size
//   - average arrangement size
//   - all arrangement sizes
//   - number of merges
//   - merge timings
//   - number of batches received
//   - max/min/average batch sizes
//   - list of all batch sizes
//   - # of traces
//   - creation time
//   - drop time
//
// - Timeline events
//   - event id (is this actually needed?)
//   - worker
//   - event
//   - when the event started
//   - when the event ended (unneeded?)
//   - event duration

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct GraphData {
    nodes: Vec<Node>,
    subgraphs: Vec<Subgraph>,
    edges: Vec<Edge>,
    palette_colors: Vec<String>,
    timeline_events: Vec<WorkerTimelineEvent>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Node {
    pub id: usize,
    pub worker: WorkerId,
    pub addr: OperatorAddr,
    pub name: String,
    pub max_activation_time: String,
    pub min_activation_time: String,
    pub average_activation_time: String,
    pub total_activation_time: String,
    pub invocations: usize,
    pub fill_color: String,
    pub text_color: String,
    pub activation_durations: Vec<ActivationDuration>,
    pub max_arrangement_size: Option<usize>,
    pub min_arrangement_size: Option<usize>,
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct ActivationDuration {
    pub activation_time: u64,
    pub activated_at: u64,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Subgraph {
    pub id: usize,
    pub worker: WorkerId,
    pub addr: OperatorAddr,
    pub name: String,
    pub max_activation_time: String,
    pub min_activation_time: String,
    pub average_activation_time: String,
    pub total_activation_time: String,
    pub invocations: usize,
    pub fill_color: String,
    pub text_color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Edge {
    pub src: OperatorAddr,
    pub dest: OperatorAddr,
    pub worker: WorkerId,
    pub channel_id: usize,
    pub edge_kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum EdgeKind {
    Normal,
    Crossing,
}
