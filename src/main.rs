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
        Command::SmokeEcho { port } => protohackers::problem0::run(port).await?,
        Command::PrimeTime { port } => protohackers::problem1::run(port).await?,
        Command::MeanToAnEnd { port } => protohackers::problem2::run(port).await?,
        Command::BudgetChat { port } => protohackers::problem3::run(port).await?,
    }

    Ok(())
}
