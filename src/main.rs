mod cmd;
mod error;
mod gossipglomers;
mod protohackers;
mod tracer;

use clap::Parser;
use cmd::*;
pub use error::{Error, Result};
use protohackers::{run_server, run_server_with_state};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracer::setup_simple_tracing();
    let args = Args::parse();
    match args.cmd {
        Command::SmokeEcho { port } => protohackers::problem0::run(port).await?,
        Command::PrimeTime { port } => {
            run_server(port, protohackers::problem1::handle_client).await?
        }
        Command::MeanToAnEnd { port } => protohackers::problem2::run(port).await?,
        Command::BudgetChat { port } => protohackers::problem3::run(port).await?,
        Command::BudgetChatExample { port } => {
            let room = protohackers::problem3::Room::new();
            run_server_with_state(port, room, protohackers::problem3::handle_client).await?
        }
        // UDP example
        Command::UnusualDatabase { port } => protohackers::problem4::run(port).await?,
        Command::ModInMiddle { port } => protohackers::problem5::run(port).await?,
        Command::SpeedDaemon { port } => protohackers::problem6::run(port).await?,
        // Custom reliable transport protocol built on UDP
        Command::LineReversal { port } => protohackers::problem7::run(port).await?,
        Command::All => {
            let mut handles = Vec::new();

            // Helper to spawn with name
            macro_rules! spawn_server {
                ($name:expr, $port:expr, $future:expr) => {{
                    let handle = tokio::spawn(async move {
                        eprintln!("Starting {} on port {}", $name, $port);
                        if let Err(e) = $future.await {
                            eprintln!("{} crashed: {}", $name, e);
                        }
                    });
                    handles.push(handle);
                }};
            }

            // TCP
            spawn_server!("Smoke Echo", 3000, protohackers::problem0::run(3000));
            spawn_server!(
                "Prime Time",
                3001,
                run_server(3001, protohackers::problem1::handle_client)
            );
            spawn_server!("Mean to an End", 3002, protohackers::problem2::run(3002));
            spawn_server!("Budget Chat", 3003, protohackers::problem3::run(3003));
            spawn_server!("Budget Chat (stateful)", 3008, async {
                let room = protohackers::problem3::Room::new();
                run_server_with_state(3008, room, protohackers::problem3::handle_client).await
            });
            spawn_server!("MITM", 3005, protohackers::problem5::run(3005));
            spawn_server!("Speed Daemon", 3006, protohackers::problem6::run(3006));

            // UDP
            spawn_server!("Unusual DB", 3004, protohackers::problem4::run(3004));
            spawn_server!("Line Reversal", 3007, protohackers::problem7::run(3007));

            // Wait for any to fail (or Ctrl+C)
            for handle in handles {
                let _ = handle.await;
            }
        }
    }

    Ok(())
}
