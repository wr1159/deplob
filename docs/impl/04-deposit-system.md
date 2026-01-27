# Step 4: Deposit System

## Overview

The deposit system allows users to:

1. Generate a commitment hiding their deposit details
2. Create an SP1 proof of valid commitment
3. Deposit tokens into the shielded pool

## 4.1 Deposit Flow

```
User                          SP1 Program                    Smart Contract
  |                               |                               |
  |-- Generate secrets ---------> |                               |
  |   (nullifier_note, secret)    |                               |
  |                               |                               |
  |-- Compute commitment -------> |                               |
  |   commitment = H(n,s,t,a)     |                               |
  |                               |                               |
  |-- Generate proof -----------> |                               |
  |                          Verify commitment                    |
  |                          is correctly formed                  |
  |<-- proof --------------------|                               |
  |                               |                               |
  |-- deposit(commitment, token, amount, proof) ----------------> |
  |                               |                          Verify proof
  |                               |                          Transfer tokens
  |                               |                          Insert commitment
  |                               |                          Emit Deposit event
  |<-- tx receipt ------------------------------------------------|
```

## 4.2 SP1 Deposit Program

### Program Inputs/Outputs

| Type | Name | Description |
|------|------|-------------|
| Private | `nullifier_note` | Random 32 bytes |
| Private | `secret` | Random 32 bytes |
| Public | `token` | Token address (20 bytes) |
| Public | `amount` | Deposit amount (u128) |
| Public | `commitment` | Output commitment hash |

### Implementation

`sp1-programs/deposit/program/src/main.rs`:

```rust
//! Deposit Program
//!
//! Proves that a commitment is correctly formed from private inputs.
//!
//! Public Inputs:
//!   - commitment: bytes32
//!   - token: address (bytes20)
//!   - amount: uint128
//!
//! Private Inputs:
//!   - nullifier_note: bytes32
//!   - secret: bytes32

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::commitment::CommitmentPreimage;

/// Deposit proof public values
#[derive(Debug, Clone)]
pub struct DepositPublicValues {
    pub commitment: [u8; 32],
    pub token: [u8; 20],
    pub amount: u128,
}

pub fn main() {
    // ============ Read Private Inputs ============
    let nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let secret: [u8; 32] = sp1_zkvm::io::read();

    // ============ Read Public Inputs ============
    let token: [u8; 20] = sp1_zkvm::io::read();
    let amount: u128 = sp1_zkvm::io::read();

    // ============ Compute Commitment ============
    let preimage = CommitmentPreimage::new(
        nullifier_note,
        secret,
        token,
        amount,
    );

    let commitment = preimage.commitment();

    // ============ Commit Public Outputs ============
    // These become the public values verified on-chain
    sp1_zkvm::io::commit(&commitment);
    sp1_zkvm::io::commit(&token);
    sp1_zkvm::io::commit(&amount);
}
```

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

### Script for Proof Generation

`sp1-programs/deposit/script/src/main.rs`:

```rust
//! Deposit Proof Generation Script

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolType};
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

// Include the compiled ELF
const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

// Solidity ABI encoding for public values
sol! {
    struct DepositPublicValues {
        bytes32 commitment;
        address token;
        uint256 amount;
    }
}

fn main() -> anyhow::Result<()> {
    // Initialize SP1 prover
    sp1_sdk::utils::setup_logger();
    let client = ProverClient::new();

    // ============ Prepare Inputs ============

    // Private inputs (user-generated secrets)
    let nullifier_note: [u8; 32] = rand::random();
    let secret: [u8; 32] = rand::random();

    // Public inputs
    let token = Address::from_slice(&[0xAB; 20]); // Example token address
    let amount: u128 = 1_000_000_000_000_000_000; // 1 token (18 decimals)

    // Create stdin
    let mut stdin = SP1Stdin::new();
    stdin.write(&nullifier_note);
    stdin.write(&secret);
    stdin.write(&token.as_slice());
    stdin.write(&amount);

    // ============ Execute (for testing) ============

    println!("Executing deposit program...");
    let (output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution successful! Cycles: {}", report.total_instruction_count());

    // Read outputs
    let commitment: [u8; 32] = output.read();
    let token_out: [u8; 20] = output.read();
    let amount_out: u128 = output.read();

    println!("Commitment: 0x{}", hex::encode(commitment));
    println!("Token: 0x{}", hex::encode(token_out));
    println!("Amount: {}", amount_out);

    // ============ Generate Proof ============

    // Check if we should generate a real proof
    let should_prove = env::var("GENERATE_PROOF").unwrap_or_default() == "true";

    if should_prove {
        println!("Generating SP1 proof...");

        let (pk, vk) = client.setup(ELF);
        println!("Verification key: {}", vk.bytes32());

        let proof = client.prove(&pk, stdin).run()?;
        println!("Proof generated!");

        // Encode public values for Solidity
        let public_values = DepositPublicValues {
            commitment: commitment.into(),
            token: token,
            amount: U256::from(amount),
        };

        let encoded_public_values = DepositPublicValues::abi_encode(&public_values);

        // Save proof artifacts
        std::fs::write("deposit_proof.bin", proof.bytes())?;
        std::fs::write("deposit_public_values.bin", encoded_public_values)?;
        std::fs::write("deposit_vkey.txt", vk.bytes32())?;

        println!("Proof artifacts saved!");
    }

    // ============ Save User Data ============

    // User must save these to withdraw later!
    let user_data = serde_json::json!({
        "nullifier_note": hex::encode(nullifier_note),
        "secret": hex::encode(secret),
        "commitment": hex::encode(commitment),
        "token": hex::encode(token.as_slice()),
        "amount": amount.to_string(),
    });

    std::fs::write("deposit_note.json", serde_json::to_string_pretty(&user_data)?)?;
    println!("\nIMPORTANT: Save deposit_note.json securely! Required for withdrawal.");

    Ok(())
}
```

`sp1-programs/deposit/script/build.rs`:

```rust
use sp1_helper::build_program_with_args;

fn main() {
    build_program_with_args("../program", Default::default());
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
alloy-primitives = { workspace = true }
alloy-sol-types = { workspace = true }
serde_json = "1.0"
hex = { workspace = true }
anyhow = { workspace = true }
rand = { workspace = true }

[build-dependencies]
sp1-helper = { workspace = true }
```

## 4.3 Contract Integration

The deposit function in `DePLOB.sol` (already created in Step 3):

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
    // The proof verifies that commitment = H(nullifier_note || secret || token || amount)
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

## 4.4 TypeScript Client Library

`frontend/src/utils/deposit.ts`:

```typescript
import { ethers } from 'ethers';

export interface DepositNote {
  nullifierNote: string;  // hex
  secret: string;         // hex
  commitment: string;     // hex
  token: string;          // address
  amount: string;         // wei string
  leafIndex?: number;
}

/**
 * Generate random 32 bytes
 */
function randomBytes32(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(32));
}

/**
 * Poseidon hash (simplified - use actual implementation)
 */
async function poseidonHash(inputs: Uint8Array[]): Promise<Uint8Array> {
  // TODO: Use actual Poseidon implementation
  // For now, use keccak256 as placeholder
  const concatenated = new Uint8Array(inputs.reduce((acc, arr) => acc + arr.length, 0));
  let offset = 0;
  for (const arr of inputs) {
    concatenated.set(arr, offset);
    offset += arr.length;
  }
  return ethers.getBytes(ethers.keccak256(concatenated));
}

/**
 * Create a deposit note with secrets and commitment
 */
export async function createDepositNote(
  token: string,
  amount: bigint
): Promise<DepositNote> {
  const nullifierNote = randomBytes32();
  const secret = randomBytes32();

  // Pad token address to 32 bytes
  const tokenBytes = new Uint8Array(32);
  tokenBytes.set(ethers.getBytes(token), 12);

  // Convert amount to 32 bytes
  const amountBytes = new Uint8Array(32);
  const amountHex = amount.toString(16).padStart(32, '0');
  amountBytes.set(ethers.getBytes('0x' + amountHex), 0);

  // Compute commitment = H(nullifierNote, secret, token, amount)
  const commitment = await poseidonHash([
    nullifierNote,
    secret,
    tokenBytes,
    amountBytes,
  ]);

  return {
    nullifierNote: ethers.hexlify(nullifierNote),
    secret: ethers.hexlify(secret),
    commitment: ethers.hexlify(commitment),
    token,
    amount: amount.toString(),
  };
}

/**
 * Generate deposit proof using SP1
 */
export async function generateDepositProof(
  note: DepositNote
): Promise<{ proof: string; publicValues: string }> {
  // Call SP1 proof generation service
  const response = await fetch('/api/prove/deposit', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      nullifierNote: note.nullifierNote,
      secret: note.secret,
      token: note.token,
      amount: note.amount,
    }),
  });

  if (!response.ok) {
    throw new Error('Proof generation failed');
  }

  return response.json();
}

/**
 * Execute deposit transaction
 */
export async function executeDeposit(
  contract: ethers.Contract,
  note: DepositNote,
  proof: string
): Promise<ethers.TransactionReceipt> {
  const tx = await contract.deposit(
    note.commitment,
    note.token,
    note.amount,
    proof
  );

  const receipt = await tx.wait();

  // Extract leaf index from event
  const depositEvent = receipt.logs.find(
    (log: any) => log.fragment?.name === 'Deposit'
  );
  if (depositEvent) {
    note.leafIndex = Number(depositEvent.args.leafIndex);
  }

  return receipt;
}

/**
 * Save deposit note to local storage (encrypted in production!)
 */
export function saveDepositNote(note: DepositNote, password: string): void {
  // TODO: Encrypt with password before saving
  const notes = JSON.parse(localStorage.getItem('deplob_notes') || '[]');
  notes.push(note);
  localStorage.setItem('deplob_notes', JSON.stringify(notes));
}

/**
 * Load deposit notes from local storage
 */
export function loadDepositNotes(password: string): DepositNote[] {
  // TODO: Decrypt with password
  return JSON.parse(localStorage.getItem('deplob_notes') || '[]');
}
```

## 4.5 End-to-End Test

`contracts/test/Deposit.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract DepositTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public teeOperator = makeAddr("tee");

    function setUp() public {
        verifier = new MockSP1Verifier();
        deplob = new DePLOB(
            address(verifier),
            bytes32(uint256(1)),
            bytes32(uint256(2)),
            bytes32(uint256(3)),
            bytes32(uint256(4)),
            teeOperator
        );

        token = new ERC20Mock("Test", "TST");
        deplob.addSupportedToken(address(token));

        token.mint(alice, 1000 ether);
        vm.prank(alice);
        token.approve(address(deplob), type(uint256).max);
    }

    function test_DepositSuccess() public {
        bytes32 commitment = keccak256("commitment1");
        uint256 amount = 100 ether;

        uint256 balanceBefore = token.balanceOf(alice);

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        // Check balance transferred
        assertEq(token.balanceOf(alice), balanceBefore - amount);
        assertEq(token.balanceOf(address(deplob)), amount);

        // Check commitment recorded
        assertTrue(deplob.isKnownRoot(deplob.getLastRoot()));
    }

    function test_DepositMultiple() public {
        bytes32 commitment1 = keccak256("commitment1");
        bytes32 commitment2 = keccak256("commitment2");
        bytes32 commitment3 = keccak256("commitment3");

        vm.startPrank(alice);
        deplob.deposit(commitment1, address(token), 10 ether, "");
        deplob.deposit(commitment2, address(token), 20 ether, "");
        deplob.deposit(commitment3, address(token), 30 ether, "");
        vm.stopPrank();

        assertEq(token.balanceOf(address(deplob)), 60 ether);
    }

    function test_DepositEmitsEvent() public {
        bytes32 commitment = keccak256("commitment1");
        uint256 amount = 100 ether;

        vm.expectEmit(true, true, false, true);
        emit DePLOB.Deposit(commitment, 0, block.timestamp);

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");
    }

    function testFail_DepositZeroAmount() public {
        bytes32 commitment = keccak256("commitment1");

        vm.prank(alice);
        deplob.deposit(commitment, address(token), 0, "");
    }

    function testFail_DepositDuplicateCommitment() public {
        bytes32 commitment = keccak256("commitment1");

        vm.startPrank(alice);
        deplob.deposit(commitment, address(token), 100 ether, "");
        deplob.deposit(commitment, address(token), 100 ether, ""); // Should fail
        vm.stopPrank();
    }
}
```

## 4.6 Build and Test Commands

```bash
# Build SP1 deposit program
cd sp1-programs/deposit/program
cargo prove build

# Run deposit script (execute only)
cd ../script
cargo run --release

# Generate actual proof
GENERATE_PROOF=true cargo run --release

# Run Foundry tests
cd ../../../contracts
forge test --match-contract DepositTest -vvv
```

## 4.7 Checklist

- [ ] SP1 deposit program compiles
- [ ] Deposit script generates valid commitment
- [ ] Commitment matches expected hash
- [ ] SP1 proof generates successfully
- [ ] Contract accepts valid deposit
- [ ] Contract rejects duplicate commitments
- [ ] Contract rejects unsupported tokens
- [ ] Events emitted correctly
- [ ] Merkle tree updated correctly
- [ ] TypeScript client generates valid notes
