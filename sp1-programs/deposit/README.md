# DePLOB Deposit System

The deposit system allows users to privately deposit tokens into the shielded pool by generating a commitment that hides the deposit details.

## Overview

### Deposit Flow

```
User                          SP1 Program                    Smart Contract
  │                               │                               │
  │── Generate secrets ─────────► │                               │
  │   (nullifier_note, secret)    │                               │
  │                               │                               │
  │── Compute commitment ───────► │                               │
  │   commitment = H(n,s,t,a)     │                               │
  │                               │                               │
  │── Generate proof ───────────► │                               │
  │                          Verify commitment                    │
  │                          is correctly formed                  │
  │◄── proof ────────────────────│                               │
  │                               │                               │
  │── deposit(commitment, token, amount, proof) ────────────────► │
  │                               │                          Verify proof
  │                               │                          Transfer tokens
  │                               │                          Insert commitment
  │                               │                          Emit Deposit event
  │◄── tx receipt ───────────────────────────────────────────────│
```

### Commitment Scheme

```
commitment = keccak256(nullifier_note || secret || token || amount)
nullifier  = keccak256(nullifier_note)
```

- **nullifier_note**: Random 32 bytes (user generates)
- **secret**: Random 32 bytes (user generates)
- **token**: Token contract address (20 bytes, padded to 32)
- **amount**: Deposit amount (u128, padded to 32 bytes)

The nullifier is revealed when withdrawing to prevent double-spending.

## Directory Structure

```
deposit/
├── program/                    # SP1 zkVM program
│   ├── src/
│   │   └── main.rs            # Deposit proof circuit
│   ├── elf/                   # Compiled ELF binary
│   └── Cargo.toml
├── script/                    # Proof generation scripts
│   ├── src/
│   │   ├── main.rs           # Main proof generator
│   │   └── bin/
│   │       └── generate_test_data.rs  # FFI test helper
│   └── Cargo.toml
└── README.md
```

## Prerequisites

### 1. Install SP1

```bash
curl -L https://sp1up.succinct.xyz | bash
sp1up
```

### 2. Verify Installation

```bash
cargo prove --version
# Expected: cargo-prove sp1 (version info)
```

## Building

### Build the SP1 Program

```bash
cd sp1-programs/deposit/program

# Build for zkVM
cargo prove build

# Copy ELF to expected location
mkdir -p elf
cp ../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/deposit-program \
   elf/riscv32im-succinct-zkvm-elf
```

### Build the Script

```bash
cd sp1-programs/deposit/script
cargo build --release
```

## Running

### Execute Without Proof (Fast, for Testing)

```bash
cd sp1-programs/deposit/script
cargo run --release
```

**Output:**
```
=== Deposit Proof Generation ===

Private Inputs:
  nullifier_note: 0x061799f9...
  secret: 0xa5a5d63d...

Public Inputs:
  token: 0xababab...
  amount: 1000000000000000000 wei

Expected Commitment: 0xf78207c9...

--- Executing SP1 Program ---
Execution successful! Cycles: 38519

SP1 Program Outputs:
  commitment: 0xf78207c9...
  token: 0xababab...
  amount: 1000000000000000000

All outputs verified!

=== IMPORTANT ===
Save 'deposit_note.json' securely!
```

This generates `deposit_note.json`:
```json
{
  "nullifier_note": "061799f9581a7ef0920795171a046228de4fcaa43089a56f5b38fbb5b4e20cbc",
  "secret": "a5a5d63d4c16c19e657ce6963a805395443fe7c962fd9b1b5f27e3b00f0ab4a1",
  "commitment": "f78207c94935c31d31b0532d08fd9b931cd2a9468fc54ef61907a215eba3619c",
  "token": "abababababababababababababababababababab",
  "amount": "1000000000000000000"
}
```

**⚠️ IMPORTANT**: Save this file securely! You need it to withdraw your funds later.

### Generate Real ZK Proof (Slow)

```bash
cd sp1-programs/deposit/script
GENERATE_PROOF=true cargo run --release
```

This generates:
- `deposit_proof.bin` - The ZK proof bytes
- `deposit_public_values.bin` - ABI-encoded public values
- `deposit_vkey.txt` - Verification key

**Note**: Proof generation can take several minutes depending on your hardware.

## Testing

### Run Rust Tests (deplob-core)

```bash
cd sp1-programs/lib/deplob-core
cargo test
```

### Run Foundry Tests (Smart Contracts)

```bash
cd contracts

# Run all tests
forge test -vv

# Run only deposit tests
forge test --match-contract Deposit -vv

# Run E2E tests with FFI
forge test --match-test test_DepositWithFFI -vvvv
```

### E2E Test with FFI

The FFI test calls the Rust script from Foundry to generate real commitments:

```bash
# First, build the test data generator
cd sp1-programs/deposit/script
cargo build --release --bin generate_test_data

# Then run the E2E test
cd contracts
forge test --match-test test_DepositWithFFI -vvvv
```

**Output:**
```
Calling FFI to generate test data...
SP1-generated commitment:
0x31cd75e5b3863eb6b394b561d8d3407a956a3ec91f73da29a3bd49c234dca2c3
Verification key: 0x00959fa029c98f3402decb009a4e967718bfaa90587e9eb4d23a50c4484b9ac3
Deposit successful with SP1-generated commitment!
```

## Smart Contract Integration

### Deposit Function

```solidity
function deposit(
    bytes32 commitment,
    address token,
    uint256 amount,
    bytes calldata proof
) external nonReentrant {
    require(supportedTokens[token], "Token not supported");
    require(!commitments[commitment], "Commitment already exists");
    require(amount > 0, "Amount must be positive");

    // Verify SP1 proof
    bytes memory publicValues = abi.encode(commitment, token, amount);
    verifier.verifyProof(DEPOSIT_VKEY, publicValues, proof);

    // Transfer tokens to contract
    IERC20(token).safeTransferFrom(msg.sender, address(this), amount);

    // Insert commitment into Merkle tree
    uint256 leafIndex = _insert(commitment);
    commitments[commitment] = true;

    emit Deposit(commitment, leafIndex, block.timestamp);
}
```

### Verification Keys

Get the verification key from proof generation:

```bash
GENERATE_PROOF=true cargo run --release
# Output: Verification key: 0x00959fa029c98f3402decb009a4e967718bfaa90587e9eb4d23a50c4484b9ac3
```

Use this when deploying the DePLOB contract.

## Testing Approaches

| Approach | Speed | Use Case | Command |
|----------|-------|----------|---------|
| Mock Verifier | Fast | Development | `forge test` |
| FFI + Execute | Medium | CI | `forge test --match-test FFI` |
| Real Proofs | Slow | Production | `GENERATE_PROOF=true cargo run` |

### 1. Mock Verifier (Development)

Uses `SP1MockVerifier` which accepts empty proofs:

```solidity
deplob.deposit(commitment, token, amount, ""); // empty proof
```

### 2. FFI + Execute (CI)

Calls Rust to execute SP1 program (without proof generation):

```solidity
bytes memory result = vm.ffi(["cargo", "run", "--bin", "generate_test_data", ...]);
bytes32 commitment = vm.parseJsonBytes32(string(result), ".commitment");
```

### 3. Real Proofs (Production)

Deploy actual SP1Verifier and use generated proofs:

```solidity
// Deploy SP1VerifierGroth16 from @sp1-contracts
ISP1Verifier verifier = new SP1VerifierGroth16();

// Use real proof bytes
bytes memory proof = loadProof("deposit_proof.bin");
deplob.deposit(commitment, token, amount, proof);
```

## Performance

| Metric | Value |
|--------|-------|
| SP1 Execution Cycles | ~38,519 |
| Contract Gas (deposit) | ~294,000 |
| Proof Generation | Several minutes |

## Files Generated

| File | Description | Keep Secret? |
|------|-------------|--------------|
| `deposit_note.json` | User's secrets for withdrawal | **YES** |
| `deposit_proof.bin` | ZK proof bytes | No |
| `deposit_public_values.bin` | ABI-encoded outputs | No |
| `deposit_vkey.txt` | Verification key | No |

## Troubleshooting

### "cargo prove: command not found"

```bash
# Add SP1 to PATH
export PATH="$HOME/.sp1/bin:$PATH"

# Or reinstall
sp1up
```

### "ELF not found"

```bash
cd sp1-programs/deposit/program
cargo prove build
mkdir -p elf
cp ../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/deposit-program \
   elf/riscv32im-succinct-zkvm-elf
```

### FFI test skipped

```bash
# Build the binary first
cd sp1-programs/deposit/script
cargo build --release --bin generate_test_data
```

### Proof verification failed

Ensure the verification key matches the compiled program:
```bash
# Regenerate after any program changes
cargo prove build
GENERATE_PROOF=true cargo run --release
# Update DEPOSIT_VKEY in contract deployment
```
