# Step 5: Withdrawal System

## Overview

The withdrawal system allows users to:

1. Prove knowledge of a valid deposit commitment
2. Reveal a nullifier (without linking to deposit)
3. Withdraw tokens to any address

## 5.1 Withdrawal Flow

```
User                          SP1 Program                    Smart Contract
  |                               |                               |
  |-- Load deposit note --------> |                               |
  |   (nullifier_note, secret)    |                               |
  |                               |                               |
  |-- Get Merkle proof ---------> |                               |
  |   (from indexer/local)        |                               |
  |                               |                               |
  |-- Generate proof -----------> |                               |
  |                          Verify:                              |
  |                          - Commitment in tree                 |
  |                          - Nullifier correctly derived        |
  |                          - Amount/token match                 |
  |<-- proof --------------------|                               |
  |                               |                               |
  |-- withdraw(nullifier, recipient, token, amount, root, proof) ->|
  |                               |                          Verify proof
  |                               |                          Check nullifier unused
  |                               |                          Check root valid
  |                               |                          Mark nullifier spent
  |                               |                          Transfer tokens
  |<-- tx receipt ------------------------------------------------|
```

## 5.2 SP1 Withdrawal Program

### Program Inputs/Outputs

| Type | Name | Description |
|------|------|-------------|
| Private | `nullifier_note` | Original random value |
| Private | `secret` | Original secret |
| Private | `merkle_proof` | Proof of inclusion |
| Private | `leaf_index` | Position in tree |
| Public | `nullifier` | H(nullifier_note) |
| Public | `root` | Merkle root |
| Public | `recipient` | Withdrawal address |
| Public | `token` | Token address |
| Public | `amount` | Withdrawal amount |

### Implementation

`sp1-programs/withdraw/program/src/main.rs`:

```rust
//! Withdrawal Program
//!
//! Proves:
//! 1. Knowledge of (nullifier_note, secret) such that commitment exists in tree
//! 2. Nullifier is correctly derived from nullifier_note
//! 3. Commitment matches the claimed token and amount
//!
//! Public Inputs:
//!   - nullifier: bytes32
//!   - root: bytes32
//!   - recipient: address
//!   - token: address
//!   - amount: uint128
//!
//! Private Inputs:
//!   - nullifier_note: bytes32
//!   - secret: bytes32
//!   - merkle_proof: MerkleProof
//!   - leaf_index: u32

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::{
    commitment::CommitmentPreimage,
    merkle::{MerkleProof, TREE_DEPTH},
};

pub fn main() {
    // ============ Read Private Inputs ============
    let nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let secret: [u8; 32] = sp1_zkvm::io::read();
    let merkle_siblings: [[u8; 32]; TREE_DEPTH] = sp1_zkvm::io::read();
    let merkle_path_indices: [u8; TREE_DEPTH] = sp1_zkvm::io::read();
    let _leaf_index: u32 = sp1_zkvm::io::read();

    // ============ Read Public Inputs ============
    let expected_root: [u8; 32] = sp1_zkvm::io::read();
    let recipient: [u8; 20] = sp1_zkvm::io::read();
    let token: [u8; 20] = sp1_zkvm::io::read();
    let amount: u128 = sp1_zkvm::io::read();

    // ============ Reconstruct Commitment ============
    let preimage = CommitmentPreimage::new(
        nullifier_note,
        secret,
        token,
        amount,
    );

    let commitment = preimage.commitment();
    let nullifier = preimage.nullifier();

    // ============ Verify Merkle Proof ============
    let merkle_proof = MerkleProof {
        siblings: merkle_siblings,
        path_indices: merkle_path_indices,
    };

    let computed_root = merkle_proof.compute_root(&commitment);

    // Assert the computed root matches the expected root
    assert_eq!(
        computed_root, expected_root,
        "Merkle proof verification failed"
    );

    // ============ Commit Public Outputs ============
    sp1_zkvm::io::commit(&nullifier);
    sp1_zkvm::io::commit(&expected_root);
    sp1_zkvm::io::commit(&recipient);
    sp1_zkvm::io::commit(&token);
    sp1_zkvm::io::commit(&amount);
}
```

`sp1-programs/withdraw/program/Cargo.toml`:

```toml
[package]
name = "withdraw-program"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-zkvm = { workspace = true }
deplob-core = { path = "../../lib/deplob-core" }
```

### Script for Proof Generation

`sp1-programs/withdraw/script/src/main.rs`:

```rust
//! Withdrawal Proof Generation Script

use alloy_primitives::{Address, B256, U256};
use alloy_sol_types::{sol, SolType};
use deplob_core::merkle::{IncrementalMerkleTree, TREE_DEPTH};
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

// Solidity ABI encoding
sol! {
    struct WithdrawPublicValues {
        bytes32 nullifier;
        bytes32 root;
        address recipient;
        address token;
        uint256 amount;
    }
}

/// Deposit note loaded from file
#[derive(serde::Deserialize)]
struct DepositNote {
    nullifier_note: String,
    secret: String,
    commitment: String,
    token: String,
    amount: String,
    leaf_index: Option<u32>,
}

fn main() -> anyhow::Result<()> {
    sp1_sdk::utils::setup_logger();
    let client = ProverClient::new();

    // ============ Load Deposit Note ============
    let note_path = env::var("DEPOSIT_NOTE").unwrap_or_else(|_| "deposit_note.json".to_string());
    let note: DepositNote = serde_json::from_str(&std::fs::read_to_string(&note_path)?)?;

    let nullifier_note = hex::decode(&note.nullifier_note.trim_start_matches("0x"))?;
    let secret = hex::decode(&note.secret.trim_start_matches("0x"))?;
    let token = hex::decode(&note.token.trim_start_matches("0x"))?;
    let amount: u128 = note.amount.parse()?;
    let leaf_index = note.leaf_index.unwrap_or(0);

    // ============ Build Merkle Tree (simulate) ============
    // In production, this comes from an indexer
    let mut tree = IncrementalMerkleTree::new();

    // Insert the commitment at the correct index
    let commitment = hex::decode(&note.commitment.trim_start_matches("0x"))?;
    let commitment_arr: [u8; 32] = commitment.try_into().expect("Invalid commitment length");

    // For simulation, insert dummy leaves before our commitment
    for _ in 0..leaf_index {
        tree.insert([0u8; 32]);
    }
    tree.insert(commitment_arr);

    let merkle_proof = tree.proof(leaf_index);
    let root = tree.root;

    // ============ Withdrawal Parameters ============
    let recipient = Address::from_slice(&[0xCD; 20]); // Recipient address

    // ============ Prepare Inputs ============
    let mut stdin = SP1Stdin::new();

    // Private inputs
    let nullifier_note_arr: [u8; 32] = nullifier_note.try_into().expect("Invalid nullifier_note");
    let secret_arr: [u8; 32] = secret.try_into().expect("Invalid secret");
    let token_arr: [u8; 20] = token.try_into().expect("Invalid token");

    stdin.write(&nullifier_note_arr);
    stdin.write(&secret_arr);
    stdin.write(&merkle_proof.siblings);
    stdin.write(&merkle_proof.path_indices);
    stdin.write(&leaf_index);

    // Public inputs
    stdin.write(&root);
    stdin.write(recipient.as_slice());
    stdin.write(&token_arr);
    stdin.write(&amount);

    // ============ Execute ============
    println!("Executing withdrawal program...");
    let (output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution successful! Cycles: {}", report.total_instruction_count());

    // Read outputs
    let nullifier: [u8; 32] = output.read();
    let root_out: [u8; 32] = output.read();
    let recipient_out: [u8; 20] = output.read();
    let token_out: [u8; 20] = output.read();
    let amount_out: u128 = output.read();

    println!("Nullifier: 0x{}", hex::encode(nullifier));
    println!("Root: 0x{}", hex::encode(root_out));
    println!("Recipient: 0x{}", hex::encode(recipient_out));
    println!("Token: 0x{}", hex::encode(token_out));
    println!("Amount: {}", amount_out);

    // ============ Generate Proof ============
    let should_prove = env::var("GENERATE_PROOF").unwrap_or_default() == "true";

    if should_prove {
        println!("Generating SP1 proof...");

        let (pk, vk) = client.setup(ELF);
        println!("Verification key: {}", vk.bytes32());

        let proof = client.prove(&pk, stdin).run()?;
        println!("Proof generated!");

        // Encode public values
        let public_values = WithdrawPublicValues {
            nullifier: B256::from(nullifier),
            root: B256::from(root_out),
            recipient: Address::from_slice(&recipient_out),
            token: Address::from_slice(&token_out),
            amount: U256::from(amount_out),
        };

        let encoded = WithdrawPublicValues::abi_encode(&public_values);

        // Save artifacts
        std::fs::write("withdraw_proof.bin", proof.bytes())?;
        std::fs::write("withdraw_public_values.bin", encoded)?;
        std::fs::write("withdraw_vkey.txt", vk.bytes32())?;

        println!("Proof artifacts saved!");
    }

    Ok(())
}
```

## 5.3 Merkle Tree Indexer

For production, you need to track all deposits and build the Merkle tree.

`frontend/src/utils/merkleIndexer.ts`:

```typescript
import { ethers } from 'ethers';

interface LeafData {
  commitment: string;
  leafIndex: number;
  blockNumber: number;
}

export class MerkleIndexer {
  private provider: ethers.Provider;
  private contract: ethers.Contract;
  private leaves: Map<string, LeafData> = new Map();
  private tree: string[] = [];

  constructor(provider: ethers.Provider, contractAddress: string, abi: any) {
    this.provider = provider;
    this.contract = new ethers.Contract(contractAddress, abi, provider);
  }

  /**
   * Sync all deposit events from contract
   */
  async sync(fromBlock: number = 0): Promise<void> {
    const filter = this.contract.filters.Deposit();
    const events = await this.contract.queryFilter(filter, fromBlock);

    for (const event of events) {
      const { commitment, leafIndex } = (event as any).args;
      this.leaves.set(commitment, {
        commitment,
        leafIndex: Number(leafIndex),
        blockNumber: event.blockNumber,
      });
      this.tree[Number(leafIndex)] = commitment;
    }

    console.log(`Synced ${this.leaves.size} deposits`);
  }

  /**
   * Get Merkle proof for a commitment
   */
  getMerkleProof(commitment: string): {
    siblings: string[];
    pathIndices: number[];
  } {
    const leafData = this.leaves.get(commitment);
    if (!leafData) {
      throw new Error('Commitment not found');
    }

    const siblings: string[] = [];
    const pathIndices: number[] = [];

    let index = leafData.leafIndex;
    const DEPTH = 20;

    for (let level = 0; level < DEPTH; level++) {
      const siblingIndex = index % 2 === 0 ? index + 1 : index - 1;
      const sibling = this.getNode(level, siblingIndex);

      siblings.push(sibling);
      pathIndices.push(index % 2);

      index = Math.floor(index / 2);
    }

    return { siblings, pathIndices };
  }

  /**
   * Get node at specific level and index
   */
  private getNode(level: number, index: number): string {
    if (level === 0) {
      return this.tree[index] || ethers.ZeroHash;
    }

    const left = this.getNode(level - 1, index * 2);
    const right = this.getNode(level - 1, index * 2 + 1);

    return ethers.keccak256(ethers.concat([left, right]));
  }

  /**
   * Get current root
   */
  getCurrentRoot(): string {
    let root = ethers.ZeroHash;
    const DEPTH = 20;

    for (let level = 0; level < DEPTH; level++) {
      root = ethers.keccak256(ethers.concat([root, root]));
    }

    // Recompute with actual leaves
    return this.getNode(DEPTH, 0);
  }
}
```

## 5.4 Contract Integration

The withdraw function (already in Step 3):

```solidity
function withdraw(
    bytes32 nullifierHash,
    address payable recipient,
    address token,
    uint256 amount,
    bytes32 root,
    bytes calldata proof
) external nonReentrant {
    require(!nullifierHashes[nullifierHash], "Nullifier already spent");
    require(isKnownRoot(root), "Unknown root");
    require(supportedTokens[token], "Token not supported");

    // Verify SP1 proof
    bytes memory publicValues = abi.encode(
        nullifierHash,
        root,
        recipient,
        token,
        amount
    );
    verifier.verifyProof(WITHDRAW_VKEY, publicValues, proof);

    // Mark nullifier as spent
    nullifierHashes[nullifierHash] = true;

    // Transfer tokens
    IERC20(token).safeTransfer(recipient, amount);

    emit Withdrawal(recipient, nullifierHash, address(0), 0);
}
```

## 5.5 Relayer Support (Optional)

For enhanced privacy, users can submit withdrawals through a relayer.

`contracts/src/DePLOB.sol` (add to existing):

```solidity
/// @notice Withdraw via relayer (relayer pays gas, takes fee)
function withdrawViaRelayer(
    bytes32 nullifierHash,
    address payable recipient,
    address token,
    uint256 amount,
    bytes32 root,
    uint256 relayerFee,
    address relayer,
    bytes calldata proof
) external nonReentrant {
    require(!nullifierHashes[nullifierHash], "Nullifier already spent");
    require(isKnownRoot(root), "Unknown root");
    require(supportedTokens[token], "Token not supported");
    require(relayerFee < amount, "Fee exceeds amount");

    // Verify proof includes relayer and fee
    bytes memory publicValues = abi.encode(
        nullifierHash,
        root,
        recipient,
        token,
        amount,
        relayerFee,
        relayer
    );
    verifier.verifyProof(WITHDRAW_VKEY, publicValues, proof);

    nullifierHashes[nullifierHash] = true;

    // Pay relayer
    IERC20(token).safeTransfer(relayer, relayerFee);

    // Send remainder to recipient
    IERC20(token).safeTransfer(recipient, amount - relayerFee);

    emit Withdrawal(recipient, nullifierHash, relayer, relayerFee);
}
```

## 5.6 TypeScript Client

`frontend/src/utils/withdraw.ts`:

```typescript
import { ethers } from 'ethers';
import { DepositNote } from './deposit';
import { MerkleIndexer } from './merkleIndexer';

export interface WithdrawParams {
  note: DepositNote;
  recipient: string;
  relayer?: string;
  relayerFee?: bigint;
}

/**
 * Generate withdrawal proof
 */
export async function generateWithdrawProof(
  params: WithdrawParams,
  indexer: MerkleIndexer
): Promise<{
  nullifier: string;
  root: string;
  proof: string;
}> {
  const { note, recipient } = params;

  // Get Merkle proof
  const merkleProof = indexer.getMerkleProof(note.commitment);
  const root = indexer.getCurrentRoot();

  // Call SP1 prover service
  const response = await fetch('/api/prove/withdraw', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      nullifierNote: note.nullifierNote,
      secret: note.secret,
      token: note.token,
      amount: note.amount,
      leafIndex: note.leafIndex,
      merkleSiblings: merkleProof.siblings,
      merklePathIndices: merkleProof.pathIndices,
      recipient,
      root,
    }),
  });

  if (!response.ok) {
    throw new Error('Proof generation failed');
  }

  return response.json();
}

/**
 * Execute withdrawal
 */
export async function executeWithdraw(
  contract: ethers.Contract,
  params: WithdrawParams,
  proof: { nullifier: string; root: string; proof: string }
): Promise<ethers.TransactionReceipt> {
  const { note, recipient } = params;

  const tx = await contract.withdraw(
    proof.nullifier,
    recipient,
    note.token,
    note.amount,
    proof.root,
    proof.proof
  );

  return tx.wait();
}

/**
 * Execute withdrawal via relayer
 */
export async function executeWithdrawViaRelayer(
  relayerUrl: string,
  params: WithdrawParams,
  proof: { nullifier: string; root: string; proof: string }
): Promise<string> {
  const response = await fetch(`${relayerUrl}/relay`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      nullifier: proof.nullifier,
      recipient: params.recipient,
      token: params.note.token,
      amount: params.note.amount,
      root: proof.root,
      relayerFee: params.relayerFee?.toString(),
      proof: proof.proof,
    }),
  });

  if (!response.ok) {
    throw new Error('Relayer submission failed');
  }

  const { txHash } = await response.json();
  return txHash;
}
```

## 5.7 Tests

`contracts/test/Withdraw.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract WithdrawTest is Test {
    DePLOB public deplob;
    MockSP1Verifier public verifier;
    ERC20Mock public token;

    address public alice = makeAddr("alice");
    address public bob = makeAddr("bob");
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

        // Setup Alice with tokens
        token.mint(alice, 1000 ether);
        vm.prank(alice);
        token.approve(address(deplob), type(uint256).max);
    }

    function test_WithdrawAfterDeposit() public {
        // Deposit
        bytes32 commitment = keccak256("commitment1");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        // Withdraw to Bob
        bytes32 nullifier = keccak256("nullifier1");
        bytes32 root = deplob.getLastRoot();

        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");

        // Verify
        assertEq(token.balanceOf(bob), amount);
        assertTrue(deplob.isSpentNullifier(nullifier));
    }

    function testFail_WithdrawDoubleSpend() public {
        bytes32 commitment = keccak256("commitment1");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        bytes32 nullifier = keccak256("nullifier1");
        bytes32 root = deplob.getLastRoot();

        // First withdrawal
        deplob.withdraw(nullifier, payable(bob), address(token), amount, root, "");

        // Second withdrawal with same nullifier should fail
        deplob.withdraw(nullifier, payable(alice), address(token), amount, root, "");
    }

    function testFail_WithdrawInvalidRoot() public {
        bytes32 commitment = keccak256("commitment1");
        uint256 amount = 100 ether;

        vm.prank(alice);
        deplob.deposit(commitment, address(token), amount, "");

        bytes32 nullifier = keccak256("nullifier1");
        bytes32 fakeRoot = keccak256("fake root");

        deplob.withdraw(nullifier, payable(bob), address(token), amount, fakeRoot, "");
    }

    function test_WithdrawPreservesPrivacy() public {
        // Multiple deposits
        bytes32 commitment1 = keccak256("c1");
        bytes32 commitment2 = keccak256("c2");
        bytes32 commitment3 = keccak256("c3");

        vm.startPrank(alice);
        deplob.deposit(commitment1, address(token), 100 ether, "");
        deplob.deposit(commitment2, address(token), 100 ether, "");
        deplob.deposit(commitment3, address(token), 100 ether, "");
        vm.stopPrank();

        bytes32 root = deplob.getLastRoot();

        // Withdraw to different addresses - no link to deposits visible
        bytes32 nullifier1 = keccak256("n1");
        bytes32 nullifier2 = keccak256("n2");

        address charlie = makeAddr("charlie");
        address dave = makeAddr("dave");

        deplob.withdraw(nullifier1, payable(charlie), address(token), 100 ether, root, "");
        deplob.withdraw(nullifier2, payable(dave), address(token), 100 ether, root, "");

        assertEq(token.balanceOf(charlie), 100 ether);
        assertEq(token.balanceOf(dave), 100 ether);
    }
}
```

## 5.8 Build and Test

```bash
# Build SP1 withdraw program
cd sp1-programs/withdraw/program
cargo prove build

# Test withdrawal script
cd ../script
DEPOSIT_NOTE=../../deposit/script/deposit_note.json cargo run --release

# Generate proof
DEPOSIT_NOTE=../../deposit/script/deposit_note.json GENERATE_PROOF=true cargo run --release

# Run Foundry tests
cd ../../../contracts
forge test --match-contract WithdrawTest -vvv
```

## 5.9 Checklist

- [ ] SP1 withdraw program compiles
- [ ] Merkle proof verification works
- [ ] Nullifier correctly derived
- [ ] Withdrawal to any address works
- [ ] Double-spend prevented
- [ ] Invalid root rejected
- [ ] Merkle indexer syncs events
- [ ] Relayer support works (optional)
