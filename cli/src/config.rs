use clap::Args;

/// Shared on-chain config arguments used by deposit, withdraw, etc.
#[derive(Args, Clone)]
pub struct ChainArgs {
    /// JSON-RPC URL (e.g. http://localhost:8545 or https://sepolia.infura.io/v3/<key>)
    #[arg(long, env = "ETH_RPC_URL")]
    pub rpc_url: String,

    /// DePLOB contract address (hex)
    #[arg(long, env = "DEPLOB_ADDRESS")]
    pub contract: String,

    /// Private key for signing transactions (hex, with or without 0x prefix)
    #[arg(long, env = "PRIVATE_KEY")]
    pub private_key: String,
}
