# Step 6: Order Creation System

## Overview

The order creation system allows users to:
1. Create an encrypted limit order backed by a deposit
2. Prove ownership of the deposit without revealing it
3. Submit the encrypted order to the TEE via the smart contract

## 6.1 Order Creation Flow

```
User                          SP1 Program                    Smart Contract              TEE
  |                               |                               |                        |
  |-- Create order data --------> |                               |                        |
  |   (price, qty, side)          |                               |                        |
  |                               |                               |                        |
  |-- Link to deposit ----------> |                               |                        |
  |   (deposit commitment)        |                               |                        |
  |                               |                               |                        |
  |-- Generate order commitment ->|                               |                        |
  |                               |                               |                        |
  |-- Encrypt order for TEE ----> |                               |                        |
  |                               |                               |                        |
  |-- Generate proof -----------> |                               |                        |
  |                          Verify:                              |                        |
  |                          - User owns deposit                  |                        |
  |                          - Order commitment valid             |                        |
  |<-- proof --------------------|                               |                        |
  |                               |                               |                        |
  |-- createOrder(orderCommitment, depositNullifier, encOrder, proof) -->|                |
  |                               |                          Verify proof                  |
  |                               |                          Lock deposit                  |
  |                               |                          Emit event ------------------->|
  |                               |                               |                   Decrypt
  |                               |                               |                   Add to book
  |<-- tx receipt ------------------------------------------------|                        |
```

## 6.2 Order Data Structures

### Order Structure

```rust
pub struct Order {
    /// Price in base units (e.g., wei per token)
    pub price: u128,
    /// Quantity to trade
    pub quantity: u128,
    /// Buy or Sell
    pub side: OrderSide,
    /// Token being offered (sold)
    pub token_in: [u8; 20],
    /// Token being requested (bought)
    pub token_out: [u8; 20],
}

pub enum OrderSide {
    Buy = 0,
    Sell = 1,
}
```

### Order Commitment

```
order_commitment = H(
    order_nullifier_note ||
    order_secret ||
    price ||
    quantity ||
    side ||
    token_in ||
    token_out ||
    deposit_commitment
)
```

## 6.3 SP1 Order Creation Program

`sp1-programs/create-order/program/src/main.rs`:

```rust
//! Create Order Program
//!
//! Proves:
//! 1. User owns a deposit commitment in the tree
//! 2. Deposit amount covers the order
//! 3. Order commitment is correctly formed
//!
//! Public Inputs:
//!   - order_commitment: bytes32
//!   - deposit_nullifier: bytes32 (locks deposit for this order)
//!
//! Private Inputs:
//!   - deposit_nullifier_note, deposit_secret, deposit_token, deposit_amount
//!   - deposit_merkle_proof
//!   - order_nullifier_note, order_secret
//!   - order: (price, quantity, side, token_in, token_out)

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::{
    commitment::CommitmentPreimage,
    merkle::{MerkleProof, TREE_DEPTH},
    order::{Order, OrderSide},
};

pub fn main() {
    // ============ Read Deposit Private Inputs ============
    let deposit_nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let deposit_secret: [u8; 32] = sp1_zkvm::io::read();
    let deposit_token: [u8; 20] = sp1_zkvm::io::read();
    let deposit_amount: u128 = sp1_zkvm::io::read();

    // Merkle proof for deposit
    let merkle_siblings: [[u8; 32]; TREE_DEPTH] = sp1_zkvm::io::read();
    let merkle_path_indices: [u8; TREE_DEPTH] = sp1_zkvm::io::read();
    let merkle_root: [u8; 32] = sp1_zkvm::io::read();

    // ============ Read Order Private Inputs ============
    let order_nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let order_secret: [u8; 32] = sp1_zkvm::io::read();

    let price: u128 = sp1_zkvm::io::read();
    let quantity: u128 = sp1_zkvm::io::read();
    let side: u8 = sp1_zkvm::io::read();
    let token_in: [u8; 20] = sp1_zkvm::io::read();
    let token_out: [u8; 20] = sp1_zkvm::io::read();

    // ============ Verify Deposit Ownership ============
    let deposit_preimage = CommitmentPreimage::new(
        deposit_nullifier_note,
        deposit_secret,
        deposit_token,
        deposit_amount,
    );

    let deposit_commitment = deposit_preimage.commitment();
    let deposit_nullifier = deposit_preimage.nullifier();

    // Verify Merkle proof
    let merkle_proof = MerkleProof {
        siblings: merkle_siblings,
        path_indices: merkle_path_indices,
    };

    let computed_root = merkle_proof.compute_root(&deposit_commitment);
    assert_eq!(computed_root, merkle_root, "Invalid deposit Merkle proof");

    // ============ Verify Order Validity ============

    // Determine which token is being used from deposit
    let order_side = if side == 0 { OrderSide::Buy } else { OrderSide::Sell };

    // For a sell order: deposit_token must equal token_in, amount >= quantity
    // For a buy order: deposit_token must equal token_out (paying with token_in)
    match order_side {
        OrderSide::Sell => {
            assert_eq!(deposit_token, token_in, "Deposit token must match token_in for sell");
            assert!(deposit_amount >= quantity, "Insufficient deposit for order");
        }
        OrderSide::Buy => {
            // For buy orders, we're offering token_in to get token_out
            // The deposit should be in token_in
            assert_eq!(deposit_token, token_in, "Deposit token must match token_in for buy");
            // Calculate required amount: quantity * price
            let required = quantity.checked_mul(price).expect("Overflow");
            assert!(deposit_amount >= required, "Insufficient deposit for order");
        }
    }

    // ============ Compute Order Commitment ============
    let order = Order {
        price,
        quantity,
        side: order_side,
        token_in,
        token_out,
    };

    let order_commitment = compute_order_commitment(
        &order_nullifier_note,
        &order_secret,
        &order,
        &deposit_commitment,
    );

    // ============ Commit Public Outputs ============
    sp1_zkvm::io::commit(&order_commitment);
    sp1_zkvm::io::commit(&deposit_nullifier);
}

fn compute_order_commitment(
    nullifier_note: &[u8; 32],
    secret: &[u8; 32],
    order: &Order,
    deposit_commitment: &[u8; 32],
) -> [u8; 32] {
    use deplob_core::poseidon::poseidon_hash;

    // Convert order fields to bytes32
    let mut price_bytes = [0u8; 32];
    price_bytes[16..].copy_from_slice(&order.price.to_be_bytes());

    let mut quantity_bytes = [0u8; 32];
    quantity_bytes[16..].copy_from_slice(&order.quantity.to_be_bytes());

    let mut side_bytes = [0u8; 32];
    side_bytes[31] = order.side as u8;

    let mut token_in_bytes = [0u8; 32];
    token_in_bytes[12..].copy_from_slice(&order.token_in);

    let mut token_out_bytes = [0u8; 32];
    token_out_bytes[12..].copy_from_slice(&order.token_out);

    poseidon_hash(&[
        *nullifier_note,
        *secret,
        price_bytes,
        quantity_bytes,
        side_bytes,
        token_in_bytes,
        token_out_bytes,
        *deposit_commitment,
    ])
}
```

### Add Order Types to deplob-core

`sp1-programs/lib/deplob-core/src/order.rs`:

```rust
//! Order types for DePLOB

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum OrderSide {
    Buy = 0,
    Sell = 1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub price: u128,
    pub quantity: u128,
    pub side: OrderSide,
    pub token_in: [u8; 20],
    pub token_out: [u8; 20],
}

/// Encrypted order for TEE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedOrder {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 12],
    /// Ephemeral public key for ECDH (if using asymmetric)
    pub ephemeral_pubkey: Option<[u8; 33]>,
}

/// Order note (user saves this)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderNote {
    pub order_nullifier_note: [u8; 32],
    pub order_secret: [u8; 32],
    pub order_commitment: [u8; 32],
    pub order: Order,
    pub deposit_commitment: [u8; 32],
}
```

## 6.4 Order Encryption for TEE

`sp1-programs/lib/deplob-core/src/order_encryption.rs`:

```rust
//! Order encryption for TEE communication

use crate::encryption::{encrypt_aes_gcm, generate_nonce};
use crate::order::{EncryptedOrder, Order};
use serde::{Deserialize, Serialize};

/// Order data to encrypt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPayload {
    pub order: Order,
    pub order_commitment: [u8; 32],
    pub deposit_nullifier: [u8; 32],
    /// User's signature for authentication (optional)
    pub signature: Option<Vec<u8>>,
}

/// Encrypt order for TEE
pub fn encrypt_order_for_tee(
    payload: &OrderPayload,
    tee_shared_key: &[u8; 32],
) -> Result<EncryptedOrder, &'static str> {
    let plaintext = bincode::serialize(payload).map_err(|_| "Serialization failed")?;
    let nonce = generate_nonce();

    let ciphertext = encrypt_aes_gcm(tee_shared_key, &plaintext, &nonce)?;

    Ok(EncryptedOrder {
        ciphertext,
        nonce,
        ephemeral_pubkey: None,
    })
}
```

## 6.5 Script for Order Creation

`sp1-programs/create-order/script/src/main.rs`:

```rust
//! Create Order Proof Generation Script

use alloy_primitives::B256;
use alloy_sol_types::{sol, SolType};
use deplob_core::{
    merkle::IncrementalMerkleTree,
    order::{Order, OrderSide},
};
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

sol! {
    struct CreateOrderPublicValues {
        bytes32 orderCommitment;
        bytes32 depositNullifier;
    }
}

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
    let deposit_note: DepositNote = serde_json::from_str(&std::fs::read_to_string(&note_path)?)?;

    let deposit_nullifier_note: [u8; 32] = hex::decode(&deposit_note.nullifier_note.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let deposit_secret: [u8; 32] = hex::decode(&deposit_note.secret.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let deposit_token: [u8; 20] = hex::decode(&deposit_note.token.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let deposit_amount: u128 = deposit_note.amount.parse()?;
    let leaf_index = deposit_note.leaf_index.unwrap_or(0);

    // ============ Build Merkle Tree ============
    let deposit_commitment: [u8; 32] = hex::decode(&deposit_note.commitment.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");

    let mut tree = IncrementalMerkleTree::new();
    for _ in 0..leaf_index {
        tree.insert([0u8; 32]);
    }
    tree.insert(deposit_commitment);
    let merkle_proof = tree.proof(leaf_index);
    let merkle_root = tree.root;

    // ============ Order Parameters ============
    let order_nullifier_note: [u8; 32] = rand::random();
    let order_secret: [u8; 32] = rand::random();

    let price: u128 = 1_000_000; // 1 USDC per token (6 decimals)
    let quantity: u128 = 100_000_000_000_000_000; // 0.1 tokens
    let side: u8 = OrderSide::Sell as u8;
    let token_in = deposit_token; // Selling deposit token
    let token_out: [u8; 20] = [0xUSDC; 20]; // Example USDC address

    // ============ Prepare Inputs ============
    let mut stdin = SP1Stdin::new();

    // Deposit private inputs
    stdin.write(&deposit_nullifier_note);
    stdin.write(&deposit_secret);
    stdin.write(&deposit_token);
    stdin.write(&deposit_amount);
    stdin.write(&merkle_proof.siblings);
    stdin.write(&merkle_proof.path_indices);
    stdin.write(&merkle_root);

    // Order private inputs
    stdin.write(&order_nullifier_note);
    stdin.write(&order_secret);
    stdin.write(&price);
    stdin.write(&quantity);
    stdin.write(&side);
    stdin.write(&token_in);
    stdin.write(&token_out);

    // ============ Execute ============
    println!("Executing create order program...");
    let (output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution successful! Cycles: {}", report.total_instruction_count());

    let order_commitment: [u8; 32] = output.read();
    let deposit_nullifier: [u8; 32] = output.read();

    println!("Order Commitment: 0x{}", hex::encode(order_commitment));
    println!("Deposit Nullifier: 0x{}", hex::encode(deposit_nullifier));

    // ============ Generate Proof ============
    let should_prove = env::var("GENERATE_PROOF").unwrap_or_default() == "true";

    if should_prove {
        println!("Generating SP1 proof...");

        let (pk, vk) = client.setup(ELF);
        let proof = client.prove(&pk, stdin).run()?;

        // Save artifacts
        std::fs::write("create_order_proof.bin", proof.bytes())?;
        std::fs::write("create_order_vkey.txt", vk.bytes32())?;

        println!("Proof artifacts saved!");
    }

    // ============ Save Order Note ============
    let order_note = serde_json::json!({
        "order_nullifier_note": hex::encode(order_nullifier_note),
        "order_secret": hex::encode(order_secret),
        "order_commitment": hex::encode(order_commitment),
        "deposit_nullifier": hex::encode(deposit_nullifier),
        "price": price.to_string(),
        "quantity": quantity.to_string(),
        "side": side,
        "token_in": hex::encode(token_in),
        "token_out": hex::encode(token_out),
    });

    std::fs::write("order_note.json", serde_json::to_string_pretty(&order_note)?)?;
    println!("\nOrder note saved to order_note.json");

    Ok(())
}
```

## 6.6 TypeScript Client

`frontend/src/utils/order.ts`:

```typescript
import { ethers } from 'ethers';
import { DepositNote } from './deposit';

export enum OrderSide {
  Buy = 0,
  Sell = 1,
}

export interface OrderParams {
  price: bigint;
  quantity: bigint;
  side: OrderSide;
  tokenIn: string;
  tokenOut: string;
}

export interface OrderNote {
  orderNullifierNote: string;
  orderSecret: string;
  orderCommitment: string;
  depositNullifier: string;
  order: OrderParams;
  depositNote: DepositNote;
}

/**
 * Create an order backed by a deposit
 */
export async function createOrder(
  depositNote: DepositNote,
  orderParams: OrderParams,
  teePublicKey: string
): Promise<{
  orderNote: OrderNote;
  encryptedOrder: string;
  proof: string;
}> {
  // Generate order secrets
  const orderNullifierNote = ethers.hexlify(crypto.getRandomValues(new Uint8Array(32)));
  const orderSecret = ethers.hexlify(crypto.getRandomValues(new Uint8Array(32)));

  // Call proof generation service
  const response = await fetch('/api/prove/create-order', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      deposit: depositNote,
      orderNullifierNote,
      orderSecret,
      order: {
        price: orderParams.price.toString(),
        quantity: orderParams.quantity.toString(),
        side: orderParams.side,
        tokenIn: orderParams.tokenIn,
        tokenOut: orderParams.tokenOut,
      },
    }),
  });

  if (!response.ok) {
    throw new Error('Order proof generation failed');
  }

  const result = await response.json();

  const orderNote: OrderNote = {
    orderNullifierNote,
    orderSecret,
    orderCommitment: result.orderCommitment,
    depositNullifier: result.depositNullifier,
    order: orderParams,
    depositNote,
  };

  return {
    orderNote,
    encryptedOrder: result.encryptedOrder,
    proof: result.proof,
  };
}

/**
 * Submit order to contract
 */
export async function submitOrder(
  contract: ethers.Contract,
  orderNote: OrderNote,
  encryptedOrder: string,
  proof: string
): Promise<ethers.TransactionReceipt> {
  const tx = await contract.createOrder(
    orderNote.orderCommitment,
    orderNote.depositNullifier,
    encryptedOrder,
    proof
  );

  return tx.wait();
}
```

## 6.7 Tests

`contracts/test/CreateOrder.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract CreateOrderTest is Test {
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

    function test_CreateOrderAfterDeposit() public {
        // First deposit
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        // Create order
        bytes32 orderCommitment = keccak256("order1");
        bytes32 depositNullifier = keccak256("depositNullifier1");
        bytes memory encryptedOrder = abi.encode("encrypted order data");

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, encryptedOrder, "");

        // Verify deposit is locked
        assertTrue(deplob.orderNullifiers(depositNullifier));
    }

    function test_CreateOrderEmitsEvent() public {
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        bytes32 orderCommitment = keccak256("order1");
        bytes32 depositNullifier = keccak256("depositNullifier1");
        bytes memory encryptedOrder = abi.encode("encrypted");

        vm.expectEmit(true, false, false, true);
        emit DePLOB.OrderCreated(orderCommitment, encryptedOrder, block.timestamp);

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, encryptedOrder, "");
    }

    function testFail_CreateOrderDuplicateDeposit() public {
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        bytes32 depositNullifier = keccak256("depositNullifier1");

        // First order
        vm.prank(alice);
        deplob.createOrder(keccak256("order1"), depositNullifier, "", "");

        // Second order with same deposit should fail
        vm.prank(alice);
        deplob.createOrder(keccak256("order2"), depositNullifier, "", "");
    }
}
```

## 6.8 Checklist

- [ ] SP1 create order program compiles
- [ ] Order commitment correctly computed
- [ ] Deposit ownership verified
- [ ] Order amount vs deposit amount checked
- [ ] Order encryption works
- [ ] Contract accepts valid order
- [ ] Contract rejects duplicate deposit use
- [ ] Event emitted with encrypted order
- [ ] Order note saved for cancellation
