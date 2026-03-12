# Withdrawal System

Zero-knowledge proof system for private withdrawals from the DePLOB protocol. Proves knowledge of a deposit's secret without revealing the deposit details.

## Overview

The withdrawal circuit proves:
1. Knowledge of `(nullifier_note, secret)` that produces a commitment in the Merkle tree
2. The nullifier is correctly derived from `nullifier_note`
3. The Merkle proof is valid (commitment exists at claimed root)

### Public Inputs (Visible On-chain)
- `nullifier` - Prevents double-spending (derived from nullifier_note)
- `root` - Merkle tree root at time of withdrawal
- `recipient` - Address receiving the funds
- `token` - Token contract address
- `amount` - Withdrawal amount

### Private Inputs (Hidden)
- `nullifier_note` - Random bytes used to derive nullifier
- `secret` - Random bytes protecting the deposit
- `merkle_siblings` - Merkle proof path (20 hashes)
- `merkle_path_indices` - Proof direction flags
- `leaf_index` - Position in tree (for indexing)

## Project Structure

```
withdraw/
├── program/                 # SP1 zkVM program
│   ├── Cargo.toml
│   ├── elf/                 # Compiled RISC-V binary
│   │   └── riscv32im-succinct-zkvm-elf
│   └── src/
│       └── main.rs          # Withdrawal proof circuit
└── script/                  # Host-side scripts
    ├── Cargo.toml
    ├── build.rs
    └── src/
        ├── main.rs          # Interactive withdrawal script
        └── bin/
            └── generate_test_data.rs  # FFI test data generator
```

## Build

### 1. Build SP1 Program

```bash
cd sp1-programs/withdraw/program
~/.sp1/bin/cargo-prove prove build
```

### 2. Copy ELF to Expected Location

```bash
mkdir -p elf
cp ../../../target/elf-compilation/riscv32im-succinct-zkvm-elf/release/withdraw-program elf/riscv32im-succinct-zkvm-elf
```

### 3. Build Host Script

```bash
cd ..
cargo build --release -p withdraw-script
```

## Usage

### Demo Mode (No Files Required)

```bash
cargo run --release --bin withdraw-script
```

This generates random test data, builds a mock Merkle tree, and executes the SP1 program.

### With Real Data

```bash
cargo run --release --bin withdraw-script -- <deposit_note.json> <merkle_proof.json> <recipient_address>
```

#### Input Files

**deposit_note.json** (from deposit):
```json
{
  "nullifier_note": "0x...",
  "secret": "0x...",
  "commitment": "0x...",
  "token": "0x...",
  "amount": "1000000000000000000"
}
```

**merkle_proof.json** (from contract):
```json
{
  "siblings": ["0x...", ...],
  "path_indices": [0, 1, 0, ...],
  "root": "0x...",
  "leaf_index": 42
}
```

### Generate On-chain Proof

```bash
GENERATE_PROOF=groth16 cargo run --release --bin withdraw-script -- deposit_note.json merkle_proof.json 0x1234...

# Or Plonk (no trusted setup required)
GENERATE_PROOF=plonk cargo run --release --bin withdraw-script -- deposit_note.json merkle_proof.json 0x1234...
```

Output files:
- `withdraw_proof.bin` - Serialized proof bytes
- `withdraw_public_values.bin` - ABI-encoded public values
- `withdraw_vkey.txt` - Verification key hash

## Testing

### Unit Tests (deplob-core)

```bash
cargo test -p deplob-core
```

### Foundry E2E Tests

```bash
cd contracts
forge test --match-contract WithdrawE2E -vv
```

Test coverage:
- `test_WithdrawWithMockVerifier` - Basic flow with mock proofs
- `test_CannotDoubleSpend` - Nullifier prevents reuse
- `test_WithdrawInvalidRootReverts` - Unknown roots rejected
- `test_WithdrawWithFFI` - Full cycle with real SP1 execution
- `test_FFIGeneratedNullifierPreventsDoubleSpend` - Double-spend with FFI data

### FFI Test Data Generator

For CI/testing, generate withdrawal data via FFI:

```bash
cargo run --release --bin generate_withdraw_test_data -- <token> <amount> <recipient>

# Example:
cargo run --release --bin generate_withdraw_test_data -- \
    abababababababababababababababababababab \
    1000000000000000000 \
    1234567890123456789012345678901234567890
```

Returns JSON:
```json
{
  "commitment": "0x...",
  "nullifier_note": "0x...",
  "secret": "0x...",
  "nullifier": "0x...",
  "root": "0x...",
  "recipient": "0x...",
  "token": "0x...",
  "amount": "1000000000000000000",
  "leaf_index": 0,
  "siblings": ["0x...", ...],
  "path_indices": [0, 0, ...],
  "vkey": "0x...",
  "proof": "0x"
}
```

## Performance

| Metric | Value |
|--------|-------|
| SP1 Cycles | ~446,000 |
| Proof Generation (Groth16) | 10-30 minutes |
| On-chain Verification | ~300k gas |

## Cryptographic Details

### Nullifier Derivation
```
nullifier = keccak256(nullifier_note)
```

### Commitment Scheme
```
commitment = keccak256(nullifier_note || secret || token_padded || amount_padded)
```

### Merkle Proof Verification
- Tree depth: 20 levels (~1M leaves)
- Hash function: keccak256
- Proof: 20 sibling hashes + 20 direction bits

## Security Considerations

1. **Nullifier Uniqueness**: Each deposit can only be withdrawn once
2. **Root Validation**: Contract maintains history of valid roots
3. **Amount Binding**: Amount is part of commitment, can't be changed
4. **Token Binding**: Token address is part of commitment
5. **Privacy**: Only nullifier is revealed, not which deposit

## Integration with Smart Contract

```solidity
function withdraw(
    bytes32 nullifierHash,
    address payable recipient,
    address token,
    uint256 amount,
    bytes32 root,
    bytes calldata proof
) external;
```

The contract verifies:
1. Nullifier not already spent
2. Root is in history (isKnownRoot)
3. SP1 proof is valid
4. Then transfers tokens

## Troubleshooting

### "Merkle proof verification failed"
- Ensure the root matches a known contract root
- Verify the commitment matches the deposit
- Check path_indices match leaf position

### "Nullifier already spent"
- This deposit has already been withdrawn
- Each deposit can only be used once

### SP1 Execution Issues
```bash
# Check ELF exists
ls -la program/elf/

# Rebuild if needed
cd program && ~/.sp1/bin/cargo-prove prove build
```
