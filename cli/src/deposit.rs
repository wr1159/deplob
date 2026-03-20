use anyhow::{Context, Result};
use clap::Args;

use alloy::{
    network::EthereumWallet,
    primitives::{Address, FixedBytes, U256},
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol,
};
use deplob_core::CommitmentPreimage;

use crate::config::ChainArgs;

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IDePLOBDeposit {
        function deposit(bytes32 commitment, address token, uint256 amount) external;
    }

    #[sol(rpc)]
    interface IERC20 {
        function approve(address spender, uint256 amount) external returns (bool);
    }
}

#[derive(Args)]
pub struct DepositArgs {
    /// Token address (hex)
    #[arg(long)]
    pub token: String,

    /// Amount to deposit (in wei, decimal string)
    #[arg(long)]
    pub amount: String,

    /// Output file for the deposit note
    #[arg(long, default_value = "deposit_note.json")]
    pub note_file: String,

    #[command(flatten)]
    pub chain: ChainArgs,
}

pub async fn run(args: DepositArgs) -> Result<()> {
    let token_hex = args.token.strip_prefix("0x").unwrap_or(&args.token);
    let token_bytes: [u8; 20] = hex::decode(token_hex)
        .context("invalid token hex")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("token must be 20 bytes"))?;
    let token_addr: Address = args.token.parse().context("invalid token address")?;

    let amount: u128 = args.amount.parse().context("invalid amount")?;
    let contract_addr: Address = args.chain.contract.parse().context("invalid contract address")?;

    // Generate random secrets
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Compute commitment
    let preimage = CommitmentPreimage::new(nullifier_note, secret, token_bytes, amount);
    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    println!("Commitment: 0x{}", hex::encode(commitment));

    // Build provider with wallet
    let signer: PrivateKeySigner = args.chain.private_key.parse().context("invalid private key")?;
    let wallet = EthereumWallet::from(signer);
    let url = args.chain.rpc_url.parse().context("invalid RPC URL")?;
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(url);

    // Approve token transfer
    println!("Approving token transfer...");
    let erc20 = IERC20::IERC20Instance::new(token_addr, &provider);
    let approve_tx = erc20
        .approve(contract_addr, U256::from(amount))
        .send()
        .await
        .context("approve send failed")?;
    let approve_receipt = approve_tx.watch().await.context("approve watch failed")?;
    println!("Approve tx: 0x{}", hex::encode(approve_receipt));

    // Deposit
    println!("Depositing...");
    let deplob = IDePLOBDeposit::IDePLOBDepositInstance::new(contract_addr, &provider);
    let deposit_tx = deplob
        .deposit(FixedBytes(commitment), token_addr, U256::from(amount))
        .send()
        .await
        .context("deposit send failed")?;
    let deposit_receipt = deposit_tx.watch().await.context("deposit watch failed")?;
    println!("Deposit tx: 0x{}", hex::encode(deposit_receipt));

    // Save deposit note
    let note = serde_json::json!({
        "commitment": format!("0x{}", hex::encode(commitment)),
        "nullifier_note": format!("0x{}", hex::encode(nullifier_note)),
        "secret": format!("0x{}", hex::encode(secret)),
        "nullifier": format!("0x{}", hex::encode(nullifier)),
        "token": format!("0x{}", hex::encode(token_bytes)),
        "amount": amount.to_string(),
    });

    std::fs::write(&args.note_file, serde_json::to_string_pretty(&note)?)?;
    println!("Deposit note saved to {}", args.note_file);
    println!("\n*** Keep this file secret — needed for withdrawal and orders ***");

    Ok(())
}
