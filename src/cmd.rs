use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author = "zhaowei", version, about)]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    Protohackers {
        #[clap(subcommand)]
        case: ProtohackerCases,
    },
    Maelstrom {
        #[clap(subcommand)]
        case: MaelstromCases,
    },
    ACStor,
    Interview {
        #[clap(subcommand)]
        case: InterviewCases,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum ProtohackerCases {
    SmokeEcho {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    PrimeTime {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    MeanToAnEnd {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    BudgetChat {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    BudgetChatExample {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    UnusualDatabase {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    ModInMiddle {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    SpeedDaemon {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
    LineReversal {
        #[arg(short, long, default_value_t = default_port())]
        port: u32,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum MaelstromCases {
    Echo,
    UniqueIds,
    Broadcast,
}

fn default_port() -> u32 {
    // Default to 3000 if PORT env var is not set or invalid
    std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000)
}

#[derive(Subcommand, Debug, Clone)]
pub enum InterviewCases {
    WordCount,
}
