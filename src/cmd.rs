use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(author = "zhaowei", version, about)]
pub struct Args {
    #[clap(subcommand)]
    pub cmd: Command,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Command {
    Server01 {
        #[arg(short, long)]
        port: u32,
    },
    Server02 {
        #[arg(short, long)]
        port: u32,
    },
}
