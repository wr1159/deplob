# DePLOB — Claude Code Context

## What is DePLOB?

**DePLOB** (Decentralized Private Limit Order Book) is an NUS CP4101 Final Year Project.
It is a privacy-preserving on-chain limit order book that hides user balances and trade
details using a combination of **ZK proofs (SP1)** and a **Trusted Execution Environment (TEE)**.

Users deposit tokens into a shielded pool (like Tornado Cash), trade privately via a TEE
matching engine, and withdraw to any address — with no on-chain link between deposit and withdrawal.

---

## Architecture

Three components work together:

```
User Wallet
    │
    ├── deposit(commitment)          ──► Smart Contract (shielded pool)
    │                                        │  Incremental Merkle tree
    │                                        │  Nullifier set
    │                                        │  Token balances
    ├── [encrypted order] ──────────────────►│──► TEE Matching Engine
    │                                        │        - Decrypts orders
    │                                        │        - Maintains private order book
    │                                        │        - Matches price-time priority
    │                                        │◄── settleMatch() (TEE only)
    │
    └── withdraw(nullifier, root, proof) ──► Smart Contract
              ZK proof (SP1/Groth16)               │
              proves Merkle inclusion               └── transfer tokens
              + nullifier derivation
```

### Who does what

| Operation | Who submits | Proof required |
|-----------|-------------|----------------|
| `deposit()` | User | None — commitment is just a hash |
| `withdraw()` | User | SP1 ZK proof (Groth16/Plonk) |
| `createOrder()` | TEE only | None — `onlyTEE` modifier |
| `cancelOrder()` | TEE only | None — `onlyTEE` modifier |
| `settleMatch()` | TEE only | TEE attestation (TODO) |

**Key design decision**: Deposit needs no proof because nothing is being proven — the user just
hashes their secrets and locks tokens. Only withdrawal needs a ZK proof because it must prove
Merkle inclusion without revealing *which* deposit is being spent.

---

## Commitment Scheme

```
nullifier_note  = random [u8; 32]
secret          = random [u8; 32]
commitment      = keccak256(nullifier_note || secret || token_padded32 || amount_padded16)
nullifier       = keccak256(nullifier_note)
```

- `commitment` goes on-chain (inserted into Merkle tree)
- `nullifier` is revealed at withdrawal to prevent double-spend
- `nullifier_note` and `secret` stay secret with the user (saved in `deposit_note.json`)

---

## Project Structure

```
CP4101/
├── contracts/                      # Foundry — Solidity smart contracts
│   ├── src/
│   │   ├── DePLOB.sol              # Main shielded pool contract
│   │   ├── MerkleTreeWithHistory.sol
│   │   └── interfaces/IDePLOB.sol
│   ├── test/
│   │   ├── DePLOB.t.sol            # Unit tests (24 tests)
│   │   ├── DepositE2E.t.sol        # Deposit E2E with FFI (4 tests)
│   │   ├── WithdrawE2E.t.sol       # Withdrawal E2E with FFI (5 tests)
│   │   └── MerkleTree.t.sol        # Merkle tree tests (9 tests)
│   └── script/Deploy.s.sol
│
├── sp1-programs/
│   ├── lib/deplob-core/            # Shared Rust crypto primitives
│   │   └── src/
│   │       ├── keccak.rs           # keccak256 (EVM compatible)
│   │       ├── commitment.rs       # CommitmentPreimage, nullifier derivation
│   │       └── merkle.rs           # IncrementalMerkleTree, MerkleProof
│   │
│   ├── deposit/script/             # Deposit note generator (pure Rust, no SP1)
│   │   └── src/
│   │       ├── main.rs             # Generates deposit_note.json
│   │       └── bin/generate_test_data.rs  # FFI helper for Foundry tests
│   │
│   └── withdraw/
│       ├── program/                # SP1 zkVM circuit (RISC-V ELF)
│       │   └── src/main.rs         # Proves Merkle inclusion + nullifier
│       └── script/
│           ├── src/main.rs         # Runs/proves withdrawal
│           └── src/bin/generate_test_data.rs  # FFI helper for Foundry tests
│
├── tee/                            # TEE matching engine (future work)
└── docs/                           # Architecture and implementation docs
```

---

## Key Contracts

### `DePLOB.sol`

Constructor: `DePLOB(address verifier, bytes32 withdrawVKey, address teeOperator)`

- `verifier` — SP1 verifier contract (use `SP1MockVerifier` for tests)
- `withdrawVKey` — SP1 program hash for the withdraw circuit
- `teeOperator` — address of the TEE, the only account allowed to call order/settlement functions

Important state:
- `nullifierHashes` — spent nullifiers (prevents double-spend)
- `orderNullifiers` — deposits locked in orders
- `cancelledOrders` — cancelled order nullifiers
- `commitments` — known commitment hashes

### `MerkleTreeWithHistory.sol`

Depth-20 incremental Merkle tree (~1M leaves). Stores last 30 roots in a ring buffer
(`ROOT_HISTORY_SIZE = 30`) so withdrawals can use any recent root, not just the latest.

---

## Build & Test Commands

### Foundry (Solidity)
```bash
cd contracts

forge test                        # Run all 42 tests
forge test -vv                    # Verbose (shows logs)
forge test --match-contract DePLOBTest
forge test --match-contract WithdrawE2E
forge build
```

### Rust workspace
```bash
# Build everything
cargo build --release

# Run deposit note generator (produces deposit_note.json)
cargo run --release --bin deposit-script

# Run withdrawal script (demo mode)
cargo run --release --bin withdraw-script

# Build SP1 withdraw program (requires SP1 toolchain)
cd sp1-programs/withdraw/program
~/.sp1/bin/cargo-prove prove build
# Then copy ELF:
cp ../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/withdraw-program \
   elf/riscv32im-succinct-zkvm-elf

# Run all Rust tests
cargo test
```

### Generate real ZK proof (slow, ~10-30 min)
```bash
cd sp1-programs/withdraw/script
GENERATE_PROOF=groth16 cargo run --release --bin withdraw-script -- \
  deposit_note.json merkle_proof.json <recipient_address>
```

---

## Cryptographic Details

### Merkle Tree
- Depth: 20 (supports 2^20 ≈ 1M deposits)
- Hash: keccak256 (EVM compatible)
- Zero value at level 0: `[0u8; 32]`
- Zero at level `i`: `keccak256(zeros[i-1] || zeros[i-1])`

### SP1 Withdraw Circuit Inputs
Private: `nullifier_note`, `secret`, `merkle_siblings[20]`, `path_indices[20]`, `leaf_index`
Public: `root`, `recipient`, `token`, `amount`
Outputs (committed): `nullifier`, `root`, `recipient`, `token`, `amount`

### Proof types for on-chain verification
- `Groth16` — requires trusted setup, ~200 bytes, cheapest gas
- `Plonk` — no trusted setup, ~868 bytes, slightly more gas
- `Core` — NOT verifiable on-chain (only for local testing)

---

## What's Implemented

- [x] Cryptographic primitives (`deplob-core`)
- [x] Incremental Merkle tree (Solidity + Rust, verified compatible)
- [x] DePLOB smart contract (deposit, withdraw, createOrder, cancelOrder, settleMatch)
- [x] SP1 withdraw proof circuit
- [x] Deposit note generator (pure Rust)
- [x] Foundry tests — 42 tests, all passing
- [x] FFI E2E tests (Foundry calling Rust via `vm.ffi()`)

## What's Not Yet Implemented

- [ ] TEE matching engine (`tee/` directory, stubbed)
- [ ] Order creation ZK circuit (was removed; TEE verifies off-chain)
- [ ] TEE attestation verification in `settleMatch()`
- [ ] SP1 programs for create-order / cancel-order (commented out in Cargo.toml)
- [ ] Frontend / SDK
- [ ] Relayer for anonymous withdrawals

See `docs/impl/` for planned implementation steps (06 onwards).

---

## Important Invariants to Preserve

1. **`nullifier = keccak256(nullifier_note)`** — must match between Rust and Solidity
2. **`commitment = keccak256(nullifier_note || secret || token_padded32 || amount_padded16)`** — padding matters; token is right-padded to 32 bytes, amount as 16 bytes big-endian
3. **Merkle tree depth must be 20** in both `MerkleTreeWithHistory.sol` and `deplob-core/merkle.rs`
4. **`ROOT_HISTORY_SIZE = 30`** — withdrawal proofs must reference one of the last 30 roots
5. **Only `WITHDRAW_VKEY`** identifies the SP1 withdraw program — get it from `client.setup(ELF)` after building
6. **`onlyTEE` modifier** guards `createOrder`, `cancelOrder`, `settleMatch` — never remove this

---

## Docs Reference

- `docs/03-system-architecture.md` — full architecture diagram and trust model
- `docs/04-operations.md` — step-by-step operation flows
- `docs/impl/` — implementation specs for each module
- `sp1-programs/deposit/README.md` — deposit note commands
- `sp1-programs/withdraw/README.md` — withdrawal commands and circuit details
