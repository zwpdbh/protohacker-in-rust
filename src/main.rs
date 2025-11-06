mod cmd;
mod error;
mod maelstrom;
mod protohackers;
mod tracer;

use crate::maelstrom::*;
use clap::Parser;
use cmd::*;
pub use error::{Error, Result};
use protohacker_in_rust::tracer::setup_simple_tracing;
use protohackers::{run_server, run_server_with_state};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Protohackers { case } => {
            let _ = tracer::setup_simple_tracing();

            match case {
                ProtohackerCases::SmokeEcho { port } => protohackers::problem0::run(port).await?,
                ProtohackerCases::PrimeTime { port } => {
                    run_server(port, protohackers::problem1::handle_client).await?
                }
                ProtohackerCases::MeanToAnEnd { port } => protohackers::problem2::run(port).await?,
                ProtohackerCases::BudgetChat { port } => protohackers::problem3::run(port).await?,
                ProtohackerCases::BudgetChatExample { port } => {
                    let room = protohackers::problem3::Room::new();
                    run_server_with_state(port, room, protohackers::problem3::handle_client).await?
                }
                // UDP example
                ProtohackerCases::UnusualDatabase { port } => {
                    protohackers::problem4::run(port).await?
                }
                ProtohackerCases::ModInMiddle { port } => protohackers::problem5::run(port).await?,
                ProtohackerCases::SpeedDaemon { port } => protohackers::problem6::run(port).await?,
                // Custom reliable transport protocol built on UDP
                ProtohackerCases::LineReversal { port } => {
                    protohackers::problem7::run(port).await?
                }
            }
        }
        Command::Maelstrom { case } => {
            let _ = setup_simple_tracing();

            match case {
                MaelstromCases::Echo => {
                    let _ = run_with_node(EchoNode::new()).await?;
                }
                MaelstromCases::UniqueIds => {
                    let _ = run_with_node(UniqueIdsNode::new()).await?;
                }
                MaelstromCases::Broadcast => {
                    let _ = run_with_node(BroadcastNode::new()).await?;
                }
            }
        }
    }

    Ok(())
}
