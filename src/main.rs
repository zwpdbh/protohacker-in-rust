mod cmd;
mod error;
mod protohackers;
mod tracing;

use clap::Parser;
use cmd::*;
pub use error::{Error, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing::setup_simple_tracing();
    let args = Args::parse();
    match args.cmd {
        Command::Server01 { port } => protohackers::echo_server::run(port).await?,
        Command::Server02 { port } => protohackers::prime_time::run(port).await?,
        Command::Server03 { port } => protohackers::mean_to_end::run(port).await?,
    }

    Ok(())
}
