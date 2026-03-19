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

```text
User Wallet
    │
    ├── deposit(commitment)          ──► Smart Contract (shielded pool)
    │                                        │  Incremental Merkle tree
    │                                        │  Nullifier set
    │                                        │  Token balances
    │
    ├── POST /v1/orders ───────────────────► TEE Matching Engine
    │   (deposit secrets + order params)          - Verifies deposit inclusion
    │                                             - Maintains private order book
    │                                             - Matches price-time priority
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
| `deposit()` | User → Contract | None — commitment is just a hash |
| `withdraw()` | User → Contract | SP1 ZK proof (Groth16/Plonk) |
| `POST /v1/orders` | User → TEE | None — TEE verifies deposit secrets internally |
| `DELETE /v1/orders/:id` | User → TEE | None — TEE verifies nullifier_note preimage |
| `settleMatch()` | TEE → Contract | TEE attestation (TODO) |

**Key design decision**: Deposit needs no proof because nothing is being proven — the user just
hashes their secrets and locks tokens. Only withdrawal needs a ZK proof because it must prove
Merkle inclusion without revealing *which* deposit is being spent.

Order lifecycle (create/cancel) is handled entirely by the TEE in-memory — no on-chain
round-trip required. Only settlement hits the contract.

---

## Commitment Scheme

```text
nullifier_note  = random [u8; 32]
secret          = random [u8; 32]
commitment      = keccak256(nullifier_note || secret || token_padded32 || amount_padded32)
nullifier       = keccak256(nullifier_note)
```

Padding: token is left-padded to 32 bytes (12 zero bytes + 20-byte address);
amount is a u128 left-padded to 32 bytes (16 zero bytes + 16-byte big-endian).

- `commitment` goes on-chain (inserted into Merkle tree)
- `nullifier` is revealed at withdrawal to prevent double-spend
- `nullifier_note` and `secret` stay secret with the user (saved in `deposit_note.json`)

---

## Project Structure

```text
CP4101/
├── contracts/                      # Foundry — Solidity smart contracts
│   ├── src/
│   │   ├── DePLOB.sol              # Main shielded pool contract
│   │   ├── MerkleTreeWithHistory.sol
│   │   └── interfaces/IDePLOB.sol
│   ├── test/
│   │   ├── DePLOB.t.sol            # Unit tests (18 tests)
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
├── tee/                            # TEE matching engine (axum HTTP server)
│   └── src/
│       ├── main.rs                 # Axum server on :3000
│       ├── state.rs                # TeeState (book, locked_deposits, chain)
│       ├── types.rs                # OrderSide, OrderEntry, Trade
│       ├── chain.rs                # ChainClient trait + MockChainClient
│       ├── verification.rs         # verify_deposit_ownership, verify_deposit_covers_order
│       ├── orderbook/mod.rs        # Price-time priority BTreeMap order book
│       ├── matching/mod.rs         # add_and_match, run_matching
│       ├── settlement/mod.rs       # generate_settlement, SettlementData
│       └── routes/
│           ├── mod.rs              # Route registration
│           ├── orders.rs           # POST /v1/orders, DELETE /v1/orders/:id
│           └── health.rs           # GET /v1/health
│
├── frontend/                       # React frontend (spec in docs/impl/10; not yet created)
├── backend/                        # Node.js proof service (spec in docs/impl/10; not yet created)
└── docs/                           # Architecture and implementation docs
```

---

## Key Contracts

### `DePLOB.sol`

Constructor: `DePLOB(address verifier, bytes32 withdrawVKey, address teeOperator)`

- `verifier` — SP1 verifier contract (use `SP1MockVerifier` for tests)
- `withdrawVKey` — SP1 program hash for the withdraw circuit
- `teeOperator` — address of the TEE, the only account allowed to call `settleMatch`

Important state:

- `nullifierHashes` — spent nullifiers (prevents double-spend at withdrawal and settlement)
- `commitments` — known commitment hashes (set at deposit and settlement)

### `MerkleTreeWithHistory.sol`

Depth-20 incremental Merkle tree (~1M leaves). Stores last 30 roots in a ring buffer
(`ROOT_HISTORY_SIZE = 30`) so withdrawals can use any recent root, not just the latest.

---

## Build & Test Commands

### Foundry (Solidity)

```bash
cd contracts

forge test                        # Run all 36 tests
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

# Run TEE matching engine server (port 3000)
cargo run -p deplob-tee

# Run all Rust tests (TEE unit tests + deplob-core)
cargo test

# Run only TEE tests
cargo test -p deplob-tee

# Build SP1 withdraw program (requires SP1 toolchain)
cd sp1-programs/withdraw/program
~/.sp1/bin/cargo-prove prove build
# Then copy ELF:
cp ../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/withdraw-program \
   elf/riscv32im-succinct-zkvm-elf
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
- [x] DePLOB smart contract (deposit, withdraw, settleMatch)
- [x] SP1 withdraw proof circuit
- [x] Deposit note generator (pure Rust)
- [x] Foundry tests — 36 tests, all passing
- [x] FFI E2E tests (Foundry calling Rust via `vm.ffi()`)
- [x] TEE matching engine — axum HTTP server, price-time order book, matching, settlement
- [x] TEE routes — `POST /v1/orders`, `DELETE /v1/orders/:id`, `GET /v1/health`
- [x] Frontend + backend spec — `docs/impl/10-frontend-integration.md`

## What's Not Yet Implemented

- [ ] Frontend React app (`frontend/` — spec complete in docs/impl/10)
- [ ] Backend proof service (`backend/` — spec complete in docs/impl/10)
- [ ] TEE attestation verification in `settleMatch()`
- [ ] Relayer for anonymous withdrawals

See `docs/impl/` for implementation specs (00–10).

---

## Important Invariants to Preserve

1. **`nullifier = keccak256(nullifier_note)`** — must match between Rust and Solidity
2. **`commitment = keccak256(nullifier_note || secret || token_lpad32 || amount_lpad32)`** — padding matters; token and amount are left-padded (12/16 zero bytes prepended respectively)
3. **Merkle tree depth must be 20** in `MerkleTreeWithHistory.sol`, `deplob-core/merkle.rs`, and `MerkleIndexer.ts`
4. **`ROOT_HISTORY_SIZE = 30`** — withdrawal proofs must reference one of the last 30 roots
5. **Only `WITHDRAW_VKEY`** identifies the SP1 withdraw program — get it from `client.setup(ELF)` after building
6. **`onlyTEE` modifier** guards only `settleMatch` — order lifecycle (create/cancel) is managed in-memory by the TEE

---

## Docs Reference

- `docs/03-system-architecture.md` — full architecture diagram and trust model
- `docs/04-operations.md` — step-by-step operation flows
- `docs/impl/` — implementation specs for each module (00–10)
- `sp1-programs/deposit/README.md` — deposit note commands
- `sp1-programs/withdraw/README.md` — withdrawal commands and circuit details
