use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author = "zhaowei", version, about)]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    SmokeEcho {
        #[arg(short, long)]
        port: u32,
    },
    PrimeTime {
        #[arg(short, long)]
        port: u32,
    },
    MeanToAnEnd {
        #[arg(short, long)]
        port: u32,
    },
    BudgetChat {
        #[arg(short, long)]
        port: u32,
    },
    BudgetChatV2 {
        #[arg(short, long)]
        port: u32,
    },
    UnusualDatabase {
        #[arg(short, long)]
        port: u32,
    },
    ModInMiddle {
        #[arg(short, long)]
        port: u32,
    },
}
