# DePLOB Implementation Overview

## Technology Stack

| Component | Technology |
|-----------|------------|
| Smart Contracts | Solidity 0.8.x + **Foundry** |
| ZK Proofs | **SP1** (Succinct's zkVM) - Write proofs in Rust |
| TEE Engine | Rust + Intel SGX |
| Frontend | React + TypeScript + ethers.js |

## Project Structure

```
deplob/
├── contracts/                 # Foundry project
│   ├── src/
│   │   ├── DePLOB.sol        # Main shielded pool contract
│   │   ├── MerkleTree.sol    # Incremental Merkle Tree
│   │   ├── SP1Verifier.sol   # SP1 proof verifier
│   │   └── interfaces/
│   ├── test/                 # Solidity tests
│   ├── script/               # Deployment scripts
│   └── foundry.toml
│
├── sp1-programs/             # SP1 ZK programs (Rust)
│   ├── deposit/
│   │   ├── program/          # The ZK program
│   │   │   ├── src/main.rs
│   │   │   └── Cargo.toml
│   │   └── script/           # Proof generation script
│   │       ├── src/main.rs
│   │       └── Cargo.toml
│   ├── withdraw/
│   ├── create-order/
│   ├── cancel-order/
│   └── lib/                  # Shared Rust libraries
│       └── deplob-core/
│           ├── src/lib.rs
│           └── Cargo.toml
│
├── tee/                       # TEE Matching Engine
│   ├── src/
│   │   ├── main.rs
│   │   ├── orderbook/
│   │   ├── matching/
│   │   └── settlement/
│   └── Cargo.toml
│
├── frontend/
│   ├── src/
│   │   ├── components/
│   │   ├── hooks/
│   │   └── utils/
│   └── package.json
│
└── Cargo.toml                # Workspace root
```

## Implementation Phases

### Phase 1: Foundation (Weeks 1-2)

| Step | Document | Description |
|------|----------|-------------|
| 1 | [01-environment-setup.md](./01-environment-setup.md) | Foundry, SP1, Rust toolchain |
| 2 | [02-cryptographic-primitives.md](./02-cryptographic-primitives.md) | Poseidon hash, encryption in Rust |
| 3 | [03-smart-contract-foundation.md](./03-smart-contract-foundation.md) | Foundry project, Merkle tree, base contracts |

### Phase 2: Core Operations (Weeks 3-5)

| Step | Document | Description |
|------|----------|-------------|
| 4 | [04-deposit-system.md](./04-deposit-system.md) | SP1 deposit program + contract |
| 5 | [05-withdrawal-system.md](./05-withdrawal-system.md) | SP1 withdrawal program + contract |
| 6 | [06-order-creation.md](./06-order-creation.md) | SP1 order creation + contract |
| 7 | [07-order-cancellation.md](./07-order-cancellation.md) | SP1 order cancellation + contract |

### Phase 3: Matching Engine (Weeks 6-8)

| Step | Document | Description |
|------|----------|-------------|
| 8 | [08-order-matching.md](./08-order-matching.md) | TEE matching + settlement proofs |

### Phase 4: Integration (Weeks 9-10)

| Step | Document | Description |
|------|----------|-------------|
| 9 | [09-testing.md](./09-testing.md) | Foundry tests, SP1 tests, E2E |
| 10 | [10-frontend-integration.md](./10-frontend-integration.md) | React UI + proof generation |

## SP1 Overview

SP1 is a zkVM that lets you write ZK programs in Rust:

```rust
// SP1 Program (runs inside zkVM)
#![no_main]
sp1_zkvm::entrypoint!(main);

fn main() {
    // Read private inputs
    let secret = sp1_zkvm::io::read::<[u8; 32]>();

    // Compute
    let commitment = poseidon_hash(&secret);

    // Commit public outputs
    sp1_zkvm::io::commit(&commitment);
}
```

```rust
// Script (generates proof)
use sp1_sdk::{ProverClient, SP1Stdin};

fn main() {
    let client = ProverClient::new();
    let mut stdin = SP1Stdin::new();
    stdin.write(&secret);

    let (pk, vk) = client.setup(ELF);
    let proof = client.prove(&pk, stdin).run().unwrap();
}
```

### Why SP1?

| Feature | SP1 | Circom |
|---------|-----|--------|
| Language | Rust | Custom DSL |
| Learning curve | Easier (if you know Rust) | Steeper |
| Libraries | Full Rust ecosystem | Limited |
| Debugging | Standard Rust tools | Difficult |
| Proof size | ~300KB (Groth16 wrap available) | ~200B (Groth16) |
| Proving time | Seconds-minutes | Seconds |

## Key Dependencies

### Rust Workspace (Cargo.toml)

```toml
[workspace]
members = [
    "sp1-programs/deposit/program",
    "sp1-programs/deposit/script",
    "sp1-programs/withdraw/program",
    "sp1-programs/withdraw/script",
    "sp1-programs/lib/deplob-core",
    "tee",
]

[workspace.dependencies]
sp1-zkvm = "3.0.0"
sp1-sdk = "3.0.0"
alloy-primitives = "0.8"
alloy-sol-types = "0.8"
tiny-keccak = { version = "2.0", features = ["keccak"] }
```

### Foundry (foundry.toml)

```toml
[profile.default]
src = "src"
out = "out"
libs = ["lib"]
solc = "0.8.24"

[dependencies]
forge-std = "1.9.0"
sp1-contracts = "2.0.0"
```

## Development Workflow

```
1. Write SP1 program in Rust
       ↓
2. Test locally with SP1 executor
       ↓
3. Generate proof with SP1 prover
       ↓
4. Verify proof on-chain via SP1Verifier
       ↓
5. Write Foundry tests
       ↓
6. Deploy with forge script
```

## Milestones Checklist

### M1: Environment Ready

- [ ] Foundry installed and project initialized
- [ ] SP1 toolchain installed
- [ ] Sample SP1 program compiles and proves

### M2: Contracts Deployed

- [ ] MerkleTree contract tested
- [ ] SP1Verifier integrated
- [ ] DePLOB contract deployed to testnet

### M3: Deposit/Withdraw Working

- [ ] Deposit SP1 program complete
- [ ] Withdraw SP1 program complete
- [ ] End-to-end deposit → withdraw works

### M4: Orders Working

- [ ] Create order SP1 program complete
- [ ] Cancel order SP1 program complete
- [ ] TEE receives encrypted orders

### M5: Matching Working

- [ ] TEE matches orders correctly
- [ ] Settlement proofs generated
- [ ] On-chain settlement verified

### M6: Frontend Complete

- [ ] Wallet connection works
- [ ] Can deposit/withdraw via UI
- [ ] Order creation UI works

## Quick Start Commands

```bash
# Install Foundry
curl -L https://foundry.paradigm.xyz | bash
foundryup

# Install SP1
curl -L https://sp1.succinct.xyz | bash
sp1up

# Build contracts
cd contracts && forge build

# Build SP1 programs
cd sp1-programs/deposit/program && cargo prove build

# Generate proof
cd sp1-programs/deposit/script && cargo run --release

# Run Foundry tests
cd contracts && forge test -vvv

# Deploy
forge script script/Deploy.s.sol --rpc-url $RPC_URL --broadcast
```
