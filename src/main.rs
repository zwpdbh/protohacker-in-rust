mod cmd;
mod error;
mod protohackers;
mod tracing;

use clap::Parser;
use cmd::*;
pub use error::{Error, Result};
use protohackers::{run_server, run_server_with_state};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing::setup_simple_tracing();
    let args = Args::parse();
    match args.cmd {
        Command::SmokeEcho { port } => protohackers::problem0::run(port).await?,
        Command::PrimeTime { port } => {
            run_server(port, protohackers::problem1::handle_client).await?
        }
        Command::MeanToAnEnd { port } => protohackers::problem2::run(port).await?,
        Command::BudgetChat { port } => protohackers::problem3::run(port).await?,
        Command::BudgetChatV2 { port } => {
            let room = protohackers::problem3::Room::new();
            run_server_with_state(port, room, protohackers::problem3::handle_client).await?
        }
        Command::UnusualDatabase { port } => protohackers::problem4::run(port).await?,
        Command::ModInMiddle { port } => protohackers::problem5::run(port).await?,
    }

    Ok(())
}
