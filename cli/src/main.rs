mod cancel;
mod config;
mod deposit;
mod indexer;
mod order;
mod settlement;
mod withdraw;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "deplob", about = "DePLOB — Decentralized Private Limit Order Book CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Deposit tokens into the shielded pool
    Deposit(deposit::DepositArgs),
    /// Withdraw tokens from the shielded pool (requires ZK proof for real verifier)
    Withdraw(withdraw::WithdrawArgs),
    /// Create a limit order on the TEE matching engine
    Order(order::OrderArgs),
    /// Cancel an open order
    Cancel(cancel::CancelArgs),
    /// Retrieve new deposit note after a trade settlement
    Settlement(settlement::SettlementArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "deplob=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Deposit(args) => deposit::run(args).await,
        Commands::Withdraw(args) => withdraw::run(args).await,
        Commands::Order(args) => order::run(args).await,
        Commands::Cancel(args) => cancel::run(args).await,
        Commands::Settlement(args) => settlement::run(args).await,
    }
}
