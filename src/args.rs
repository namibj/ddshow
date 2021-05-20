use colorous::Gradient;
use std::{net::SocketAddr, num::NonZeroUsize, ops::Deref, path::PathBuf, str::FromStr};
use structopt::StructOpt;
use timely::{CommunicationConfig, Config};

/// Tools for profiling and visualizing Timely Dataflow & Differential Dataflow Programs
///
/// Set the `TIMELY_WORKER_LOG_ADDR` environmental variable to `127.0.0.1:51317` (or whatever
/// address you customized it to using `--listen`) to listen for Timely Dataflow computations
/// and the `DIFFERENTIAL_LOG_ADDR` variable to gather data on Differential Dataflow computations.
/// Set `--connections` to the number of timely workers that the target computation is using.
///
// TODO: Better docs
// TODO: Number of workers
// TODO: Save logs to file
// TODO: Process logs from file
#[derive(Debug, Clone, StructOpt)]
#[structopt(rename_all = "kebab-case")]
pub struct Args {
    /// The number of ddshow workers to run
    #[structopt(short = "w", long = "workers", default_value = "1")]
    pub workers: NonZeroUsize,

    /// The number of timely workers running in the target computation
    #[structopt(short = "c", long = "connections", default_value = "1")]
    pub timely_connections: NonZeroUsize,

    /// The address to listen for `timely` log messages on
    #[structopt(long = "address", default_value = "127.0.0.1:51317")]
    pub address: SocketAddr,

    /// Whether or not Differential Dataflow logs should be read from
    #[structopt(short = "d", long = "differential")]
    pub differential_enabled: bool,

    #[structopt(long = "differential-address", default_value = "127.0.0.1:51318")]
    pub differential_address: SocketAddr,

    /// The color palette to use for the generated graphs
    #[structopt(
        long = "palette",
        parse(try_from_str = gradient_from_str),
        possible_values = ACCEPTED_GRADIENTS,
        default_value = "inferno",
    )]
    pub palette: ThreadedGradient,

    /// The directory to generate artifacts in
    #[structopt(long = "output-dir", default_value = "dataflow-graph")]
    pub output_dir: PathBuf,

    /// The path to dump the json data to
    ///
    /// The format is currently unstable, so don't depend on it too hard
    #[structopt(long = "dump-json")]
    pub dump_json: Option<PathBuf>,

    #[structopt(long = "dump-json-v2")]
    pub dump_json_v2: Option<PathBuf>,

    /// The folder to save the target process's logs to
    #[structopt(long = "save-logs")]
    pub save_logs: Option<PathBuf>,

    #[structopt(long = "replay-logs")]
    pub replay_logs: Option<PathBuf>,

    #[structopt(long = "report-file", default_value = "report.txt")]
    pub report_file: PathBuf,

    #[structopt(long = "no-report-file")]
    pub no_report_file: bool,
}

impl Args {
    pub fn timely_config(&self) -> Config {
        let config = {
            let communication = if self.workers.get() == 1 {
                CommunicationConfig::Thread
            } else {
                CommunicationConfig::Process(self.workers.get())
            };

            Config {
                communication,
                worker: Default::default(),
            }
        };

        // TODO: Implement `Debug` for `timely::Config`
        tracing::trace!("created timely config");

        config
    }
}

macro_rules! parse_gradient {
    ($($lower:literal => $gradient:ident),* $(,)?) => {
        fn gradient_from_str(src: &str) -> Result<ThreadedGradient, String> {
            let gradient = src.to_lowercase();

            let gradient = match gradient.as_str() {
                $(
                    $lower => colorous::$gradient,
                )*

                _ => return Err(format!("unrecognized gradient '{}'", src)),
            };

            Ok(ThreadedGradient(gradient))
        }

        // TODO: Const eval over proc macro
        const ACCEPTED_GRADIENTS: &'static [&str] = &[$($lower),*];
    };
}

parse_gradient! {
    "turbo" => TURBO,
    "viridis" => VIRIDIS,
    "inferno" => INFERNO,
    "magma" => MAGMA,
    "plasma" => PLASMA,
    "cividis" => CIVIDIS,
    "warm" => WARM,
    "cool" => COOL,
    "cubehelix" => CUBEHELIX,
    "blue-green" => BLUE_GREEN,
    "blue-purple" => BLUE_PURPLE,
    "green-blue" => GREEN_BLUE,
    "orange-red" => ORANGE_RED,
    "purple-blue-green" => PURPLE_BLUE_GREEN,
    "purple-blue" => PURPLE_BLUE,
    "purple-red" => PURPLE_RED,
    "red-purple" => RED_PURPLE,
    "yellow-green-blue" => YELLOW_GREEN_BLUE,
    "yellow-green" => YELLOW_GREEN,
    "yellow-orange-brown" => YELLOW_ORANGE_BROWN,
    "yellow-orange-red" => YELLOW_ORANGE_RED,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadedGradient(Gradient);

impl Deref for ThreadedGradient {
    type Target = Gradient;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for ThreadedGradient {
    fn default() -> Self {
        Self(colorous::INFERNO)
    }
}

unsafe impl Send for ThreadedGradient {}
unsafe impl Sync for ThreadedGradient {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Output {
    Stdout,
    Stderr,
    Quiet,
}

impl Output {
    // const VALUES: &'static [&'static str] = &["stdout", "stderr", "quiet"];
}

impl FromStr for Output {
    type Err = String;

    fn from_str(src: &str) -> Result<Self, Self::Err> {
        let string = src.to_owned();

        match string.to_lowercase().as_str() {
            "stdout" => Ok(Self::Stdout),
            "stderr" => Ok(Self::Stderr),
            "quiet" => Ok(Self::Quiet),

            _ => Err(format!("invalid output type: {}", src)),
        }
    }
}
