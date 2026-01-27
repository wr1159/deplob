# Step 1: Environment Setup

## Prerequisites

- macOS / Linux (Windows via WSL2)
- 16GB+ RAM recommended for SP1 proving
- Git

## 1.1 Install Rust

```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Restart shell or run:
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version

# Add RISC-V target (required for SP1)
rustup target add riscv32im-unknown-none-elf
```

## 1.2 Install Foundry

```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash

# Run foundryup to install forge, cast, anvil, chisel
foundryup

# Verify installation
forge --version
cast --version
anvil --version
```

### Foundry Tools

| Tool | Purpose |
|------|---------|
| `forge` | Build, test, deploy contracts |
| `cast` | CLI for interacting with contracts |
| `anvil` | Local Ethereum node for testing |
| `chisel` | Solidity REPL |

## 1.3 Install SP1

```bash
# Install SP1 toolchain
curl -L https://sp1.succinct.xyz | bash

# Run sp1up to install
sp1up

# Verify installation
cargo prove --version
```

### SP1 Components

| Component | Purpose |
|-----------|---------|
| `cargo prove build` | Compile SP1 programs |
| `cargo prove` | Run proving |
| `sp1-zkvm` | Crate for writing ZK programs |
| `sp1-sdk` | Crate for generating/verifying proofs |

## 1.4 Initialize Project Structure

```bash
# Create project root
mkdir deplob && cd deplob

# Initialize Foundry project for contracts
mkdir contracts && cd contracts
forge init --no-commit
cd ..

# Create SP1 workspace structure
mkdir -p sp1-programs/{deposit,withdraw,create-order,cancel-order}/{program,script}
mkdir -p sp1-programs/lib/deplob-core

# Create TEE directory
mkdir -p tee/src/{orderbook,matching,settlement}

# Create frontend directory
mkdir -p frontend/src/{components,hooks,utils,contracts}
```

## 1.5 Setup Rust Workspace

Create `Cargo.toml` in project root:

```toml
[workspace]
resolver = "2"
members = [
    "sp1-programs/deposit/program",
    "sp1-programs/deposit/script",
    "sp1-programs/withdraw/program",
    "sp1-programs/withdraw/script",
    "sp1-programs/create-order/program",
    "sp1-programs/create-order/script",
    "sp1-programs/cancel-order/program",
    "sp1-programs/cancel-order/script",
    "sp1-programs/lib/deplob-core",
    "tee",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
# SP1
sp1-zkvm = "3.0.0"
sp1-sdk = "3.0.0"
sp1-helper = "3.0.0"

# Alloy (Ethereum types)
alloy-primitives = "0.8"
alloy-sol-types = "0.8"

# Crypto
sha2 = "0.10"
tiny-keccak = { version = "2.0", features = ["keccak"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
bincode = "1.3"
hex = "0.4"

# Async
tokio = { version = "1", features = ["full"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"
```

## 1.6 Setup Foundry Project

Update `contracts/foundry.toml`:

```toml
[profile.default]
src = "src"
out = "out"
libs = ["lib"]
solc = "0.8.24"
optimizer = true
optimizer_runs = 200
via_ir = false
ffi = true  # Required for SP1 proof verification in tests

[profile.default.fuzz]
runs = 256

[profile.ci]
fuzz = { runs = 10000 }

[rpc_endpoints]
sepolia = "${SEPOLIA_RPC_URL}"
mainnet = "${MAINNET_RPC_URL}"

[etherscan]
sepolia = { key = "${ETHERSCAN_API_KEY}" }
```

Install dependencies:

```bash
cd contracts

# Install OpenZeppelin
forge install OpenZeppelin/openzeppelin-contracts --no-commit

# Install SP1 contracts for on-chain verification
forge install succinctlabs/sp1-contracts --no-commit

# Install forge-std (usually included, but ensure latest)
forge install foundry-rs/forge-std --no-commit
```

Update `contracts/remappings.txt`:

```
@openzeppelin/=lib/openzeppelin-contracts/
@sp1-contracts/=lib/sp1-contracts/contracts/
forge-std/=lib/forge-std/src/
```

## 1.7 Create Sample SP1 Program

Test the SP1 setup with a simple program.

`sp1-programs/deposit/program/Cargo.toml`:
```toml
[package]
name = "deposit-program"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-zkvm = { workspace = true }
deplob-core = { path = "../../lib/deplob-core" }
```

`sp1-programs/deposit/program/src/main.rs`:
```rust
//! Deposit Program - proves valid commitment creation
#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    // Read inputs (will implement later)
    let value = sp1_zkvm::io::read::<u64>();

    // Simple test: square the value
    let result = value * value;

    // Commit output
    sp1_zkvm::io::commit(&result);
}
```

`sp1-programs/deposit/script/Cargo.toml`:
```toml
[package]
name = "deposit-script"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-sdk = { workspace = true }
serde = { workspace = true }
anyhow = { workspace = true }

[build-dependencies]
sp1-helper = { workspace = true }
```

`sp1-programs/deposit/script/build.rs`:
```rust
fn main() {
    sp1_helper::build_program("../program");
}
```

`sp1-programs/deposit/script/src/main.rs`:
```rust
use sp1_sdk::{ProverClient, SP1Stdin};

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

fn main() -> anyhow::Result<()> {
    // Initialize prover
    let client = ProverClient::new();

    // Setup inputs
    let mut stdin = SP1Stdin::new();
    stdin.write(&42u64);

    // Execute (without proving, for testing)
    let (output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution report: {:?}", report);

    // Read output
    let result = output.read::<u64>();
    println!("Result: {} (expected: {})", result, 42 * 42);

    // Generate actual proof (uncomment when ready)
    // let (pk, vk) = client.setup(ELF);
    // let proof = client.prove(&pk, stdin).run()?;
    // println!("Proof generated!");

    Ok(())
}
```

`sp1-programs/lib/deplob-core/Cargo.toml`:
```toml
[package]
name = "deplob-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
```

`sp1-programs/lib/deplob-core/src/lib.rs`:
```rust
//! Shared types and utilities for DePLOB SP1 programs

use serde::{Deserialize, Serialize};

/// Commitment structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub hash: [u8; 32],
}

/// Nullifier structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nullifier {
    pub hash: [u8; 32],
}

// More types will be added in later steps
```

## 1.8 Build and Test

```bash
# Build the SP1 program
cd sp1-programs/deposit/program
cargo prove build

# Run the script (execute without proving)
cd ../script
cargo run --release

# Build contracts
cd ../../../contracts
forge build

# Run contract tests
forge test
```

## 1.9 Environment Variables

Create `.env` in project root:

```bash
# Network RPCs
SEPOLIA_RPC_URL=https://eth-sepolia.g.alchemy.com/v2/YOUR_KEY
MAINNET_RPC_URL=https://eth-mainnet.g.alchemy.com/v2/YOUR_KEY

# Etherscan for verification
ETHERSCAN_API_KEY=your_etherscan_key

# Private key for deployment (use hardware wallet in production!)
DEPLOYER_PRIVATE_KEY=0x...

# SP1 Network (optional, for remote proving)
SP1_PROVER=local  # or "network" for Succinct's proving network
SP1_PRIVATE_KEY=your_sp1_network_key
```

Add to `.gitignore`:
```
.env
out/
cache/
target/
node_modules/
```

## 1.10 Verify Setup Checklist

- [ ] `rustc --version` shows 1.75+
- [ ] `forge --version` shows 0.2+
- [ ] `cargo prove --version` works
- [ ] SP1 sample program compiles: `cargo prove build`
- [ ] SP1 sample script runs: `cargo run --release`
- [ ] Foundry compiles: `forge build`
- [ ] Foundry tests pass: `forge test`

## Troubleshooting

### SP1 Build Fails
```bash
# Ensure RISC-V target is installed
rustup target add riscv32im-unknown-none-elf

# Clean and rebuild
cargo clean
cargo prove build
```

### Foundry Remappings Issue
```bash
# Regenerate remappings
forge remappings > remappings.txt
```

### Out of Memory During Proving
```bash
# SP1 proving needs significant RAM
# Use network prover for large programs
export SP1_PROVER=network
```
