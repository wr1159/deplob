# Step 7: Order Cancellation System

## Overview

The order cancellation system allows users to:
1. Cancel an existing order
2. Unlock the deposit for withdrawal or new orders
3. Notify the TEE to remove the order from the book

## 7.1 Cancellation Flow

```
User                          SP1 Program                    Smart Contract              TEE
  |                               |                               |                        |
  |-- Load order note ----------> |                               |                        |
  |   (order_nullifier_note)      |                               |                        |
  |                               |                               |                        |
  |-- Generate proof -----------> |                               |                        |
  |                          Verify:                              |                        |
  |                          - Knowledge of order preimage        |                        |
  |                          - Order commitment matches           |                        |
  |<-- proof --------------------|                               |                        |
  |                               |                               |                        |
  |-- cancelOrder(orderNullifier, orderCommitment, proof) ------->|                        |
  |                               |                          Verify proof                  |
  |                               |                          Check not cancelled           |
  |                               |                          Mark cancelled                |
  |                               |                          Emit event ------------------>|
  |                               |                               |                   Remove from book
  |                               |                               |                   Unlock deposit
  |<-- tx receipt ------------------------------------------------|                        |
```

## 7.2 SP1 Cancel Order Program

### Program Inputs/Outputs

| Type | Name | Description |
|------|------|-------------|
| Private | `order_nullifier_note` | Random value from order creation |
| Private | `order_secret` | Secret from order creation |
| Private | `order_data` | Price, quantity, side, tokens |
| Private | `deposit_commitment` | Linked deposit commitment |
| Public | `order_nullifier` | H(order_nullifier_note) |
| Public | `order_commitment` | For verification |

### Implementation

`sp1-programs/cancel-order/program/src/main.rs`:

```rust
//! Cancel Order Program
//!
//! Proves:
//! 1. Knowledge of order preimage (nullifier_note, secret, order_data)
//! 2. Order commitment is correctly derived
//! 3. Order nullifier is correctly derived
//!
//! Public Inputs:
//!   - order_nullifier: bytes32
//!   - order_commitment: bytes32
//!
//! Private Inputs:
//!   - order_nullifier_note: bytes32
//!   - order_secret: bytes32
//!   - price, quantity, side, token_in, token_out
//!   - deposit_commitment: bytes32

#![no_main]
sp1_zkvm::entrypoint!(main);

use deplob_core::{
    order::{Order, OrderSide},
    poseidon::poseidon_hash,
};

pub fn main() {
    // ============ Read Private Inputs ============
    let order_nullifier_note: [u8; 32] = sp1_zkvm::io::read();
    let order_secret: [u8; 32] = sp1_zkvm::io::read();

    let price: u128 = sp1_zkvm::io::read();
    let quantity: u128 = sp1_zkvm::io::read();
    let side: u8 = sp1_zkvm::io::read();
    let token_in: [u8; 20] = sp1_zkvm::io::read();
    let token_out: [u8; 20] = sp1_zkvm::io::read();
    let deposit_commitment: [u8; 32] = sp1_zkvm::io::read();

    // ============ Read Public Inputs ============
    let expected_order_commitment: [u8; 32] = sp1_zkvm::io::read();

    // ============ Verify Order Commitment ============
    let order = Order {
        price,
        quantity,
        side: if side == 0 { OrderSide::Buy } else { OrderSide::Sell },
        token_in,
        token_out,
    };

    let computed_commitment = compute_order_commitment(
        &order_nullifier_note,
        &order_secret,
        &order,
        &deposit_commitment,
    );

    assert_eq!(
        computed_commitment, expected_order_commitment,
        "Order commitment mismatch"
    );

    // ============ Compute Order Nullifier ============
    let order_nullifier = poseidon_hash(&[order_nullifier_note]);

    // ============ Commit Public Outputs ============
    sp1_zkvm::io::commit(&order_nullifier);
    sp1_zkvm::io::commit(&expected_order_commitment);
}

fn compute_order_commitment(
    nullifier_note: &[u8; 32],
    secret: &[u8; 32],
    order: &Order,
    deposit_commitment: &[u8; 32],
) -> [u8; 32] {
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

`sp1-programs/cancel-order/program/Cargo.toml`:

```toml
[package]
name = "cancel-order-program"
version = "0.1.0"
edition = "2021"

[dependencies]
sp1-zkvm = { workspace = true }
deplob-core = { path = "../../lib/deplob-core" }
```

## 7.3 Script for Cancellation

`sp1-programs/cancel-order/script/src/main.rs`:

```rust
//! Cancel Order Proof Generation Script

use alloy_primitives::B256;
use alloy_sol_types::{sol, SolType};
use sp1_sdk::{HashableKey, ProverClient, SP1Stdin};
use std::env;

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

sol! {
    struct CancelOrderPublicValues {
        bytes32 orderNullifier;
        bytes32 orderCommitment;
    }
}

#[derive(serde::Deserialize)]
struct OrderNote {
    order_nullifier_note: String,
    order_secret: String,
    order_commitment: String,
    price: String,
    quantity: String,
    side: u8,
    token_in: String,
    token_out: String,
    deposit_commitment: Option<String>,
}

fn main() -> anyhow::Result<()> {
    sp1_sdk::utils::setup_logger();
    let client = ProverClient::new();

    // ============ Load Order Note ============
    let note_path = env::var("ORDER_NOTE").unwrap_or_else(|_| "order_note.json".to_string());
    let order_note: OrderNote = serde_json::from_str(&std::fs::read_to_string(&note_path)?)?;

    let order_nullifier_note: [u8; 32] = hex::decode(&order_note.order_nullifier_note.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let order_secret: [u8; 32] = hex::decode(&order_note.order_secret.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let order_commitment: [u8; 32] = hex::decode(&order_note.order_commitment.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");

    let price: u128 = order_note.price.parse()?;
    let quantity: u128 = order_note.quantity.parse()?;
    let side = order_note.side;

    let token_in: [u8; 20] = hex::decode(&order_note.token_in.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");
    let token_out: [u8; 20] = hex::decode(&order_note.token_out.trim_start_matches("0x"))?
        .try_into().expect("Invalid length");

    // Get deposit commitment (from order note or deposit note)
    let deposit_commitment: [u8; 32] = if let Some(dc) = &order_note.deposit_commitment {
        hex::decode(dc.trim_start_matches("0x"))?.try_into().expect("Invalid length")
    } else {
        // Load from deposit note
        let deposit_note_path = env::var("DEPOSIT_NOTE").unwrap_or_else(|_| "deposit_note.json".to_string());
        let deposit_note: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&deposit_note_path)?)?;
        let dc_str = deposit_note["commitment"].as_str().expect("No commitment in deposit note");
        hex::decode(dc_str.trim_start_matches("0x"))?.try_into().expect("Invalid length")
    };

    // ============ Prepare Inputs ============
    let mut stdin = SP1Stdin::new();

    // Private inputs
    stdin.write(&order_nullifier_note);
    stdin.write(&order_secret);
    stdin.write(&price);
    stdin.write(&quantity);
    stdin.write(&side);
    stdin.write(&token_in);
    stdin.write(&token_out);
    stdin.write(&deposit_commitment);

    // Public inputs
    stdin.write(&order_commitment);

    // ============ Execute ============
    println!("Executing cancel order program...");
    let (output, report) = client.execute(ELF, stdin.clone()).run()?;
    println!("Execution successful! Cycles: {}", report.total_instruction_count());

    let order_nullifier: [u8; 32] = output.read();
    let commitment_out: [u8; 32] = output.read();

    println!("Order Nullifier: 0x{}", hex::encode(order_nullifier));
    println!("Order Commitment: 0x{}", hex::encode(commitment_out));

    // ============ Generate Proof ============
    let should_prove = env::var("GENERATE_PROOF").unwrap_or_default() == "true";

    if should_prove {
        println!("Generating SP1 proof...");

        let (pk, vk) = client.setup(ELF);
        let proof = client.prove(&pk, stdin).run()?;

        // Encode public values
        let public_values = CancelOrderPublicValues {
            orderNullifier: B256::from(order_nullifier),
            orderCommitment: B256::from(commitment_out),
        };

        let encoded = CancelOrderPublicValues::abi_encode(&public_values);

        // Save artifacts
        std::fs::write("cancel_order_proof.bin", proof.bytes())?;
        std::fs::write("cancel_order_public_values.bin", encoded)?;
        std::fs::write("cancel_order_vkey.txt", vk.bytes32())?;

        println!("Proof artifacts saved!");
    }

    Ok(())
}
```

## 7.4 Contract Function

Already defined in Step 3:

```solidity
function cancelOrder(
    bytes32 orderNullifier,
    bytes32 orderCommitment,
    bytes calldata proof
) external nonReentrant {
    require(!cancelledOrders[orderNullifier], "Order already cancelled");

    // Verify SP1 proof
    bytes memory publicValues = abi.encode(orderNullifier, orderCommitment);
    verifier.verifyProof(CANCEL_ORDER_VKEY, publicValues, proof);

    // Mark order as cancelled
    cancelledOrders[orderNullifier] = true;

    emit OrderCancelled(orderNullifier, block.timestamp);
}
```

## 7.5 TEE Handling

When TEE receives `OrderCancelled` event:

```rust
// tee/src/orderbook/mod.rs

impl OrderBook {
    /// Handle order cancellation from contract event
    pub fn handle_cancellation(&mut self, order_commitment: [u8; 32]) -> Option<Order> {
        // Find order by commitment
        let order = self.find_by_commitment(&order_commitment)?;

        // Remove from appropriate book
        match order.side {
            OrderSide::Buy => {
                self.bids.retain(|o| o.commitment != order_commitment);
            }
            OrderSide::Sell => {
                self.asks.retain(|o| o.commitment != order_commitment);
            }
        }

        // Mark deposit as unlocked (can be used for new order or withdrawal)
        self.unlocked_deposits.insert(order.deposit_nullifier);

        Some(order)
    }
}
```

## 7.6 TypeScript Client

`frontend/src/utils/cancelOrder.ts`:

```typescript
import { ethers } from 'ethers';
import { OrderNote } from './order';

/**
 * Generate cancellation proof
 */
export async function generateCancelProof(
  orderNote: OrderNote
): Promise<{
  orderNullifier: string;
  proof: string;
}> {
  const response = await fetch('/api/prove/cancel-order', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      orderNullifierNote: orderNote.orderNullifierNote,
      orderSecret: orderNote.orderSecret,
      orderCommitment: orderNote.orderCommitment,
      order: {
        price: orderNote.order.price.toString(),
        quantity: orderNote.order.quantity.toString(),
        side: orderNote.order.side,
        tokenIn: orderNote.order.tokenIn,
        tokenOut: orderNote.order.tokenOut,
      },
      depositCommitment: orderNote.depositNote.commitment,
    }),
  });

  if (!response.ok) {
    throw new Error('Cancel proof generation failed');
  }

  return response.json();
}

/**
 * Cancel an order
 */
export async function cancelOrder(
  contract: ethers.Contract,
  orderNote: OrderNote,
  proof: { orderNullifier: string; proof: string }
): Promise<ethers.TransactionReceipt> {
  const tx = await contract.cancelOrder(
    proof.orderNullifier,
    orderNote.orderCommitment,
    proof.proof
  );

  return tx.wait();
}

/**
 * Check if order is cancelled
 */
export async function isOrderCancelled(
  contract: ethers.Contract,
  orderNullifier: string
): Promise<boolean> {
  return contract.cancelledOrders(orderNullifier);
}
```

## 7.7 Tests

`contracts/test/CancelOrder.t.sol`:

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {DePLOB} from "../src/DePLOB.sol";
import {MockSP1Verifier} from "./mocks/MockSP1Verifier.sol";
import {ERC20Mock} from "./mocks/ERC20Mock.sol";

contract CancelOrderTest is Test {
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

    function test_CancelOrder() public {
        // Setup: deposit and create order
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        bytes32 orderCommitment = keccak256("order1");
        bytes32 depositNullifier = keccak256("depositNullifier1");

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, "", "");

        // Cancel
        bytes32 orderNullifier = keccak256("orderNullifier1");

        vm.prank(alice);
        deplob.cancelOrder(orderNullifier, orderCommitment, "");

        // Verify
        assertTrue(deplob.cancelledOrders(orderNullifier));
    }

    function test_CancelOrderEmitsEvent() public {
        // Setup
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        bytes32 orderCommitment = keccak256("order1");
        bytes32 depositNullifier = keccak256("depositNullifier1");

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, "", "");

        // Cancel with event check
        bytes32 orderNullifier = keccak256("orderNullifier1");

        vm.expectEmit(true, false, false, true);
        emit DePLOB.OrderCancelled(orderNullifier, block.timestamp);

        vm.prank(alice);
        deplob.cancelOrder(orderNullifier, orderCommitment, "");
    }

    function testFail_CancelOrderTwice() public {
        // Setup
        bytes32 depositCommitment = keccak256("deposit1");
        vm.prank(alice);
        deplob.deposit(depositCommitment, address(token), 100 ether, "");

        bytes32 orderCommitment = keccak256("order1");
        bytes32 depositNullifier = keccak256("depositNullifier1");

        vm.prank(alice);
        deplob.createOrder(orderCommitment, depositNullifier, "", "");

        bytes32 orderNullifier = keccak256("orderNullifier1");

        // First cancel
        deplob.cancelOrder(orderNullifier, orderCommitment, "");

        // Second cancel should fail
        deplob.cancelOrder(orderNullifier, orderCommitment, "");
    }
}
```

## 7.8 Build and Test

```bash
# Build SP1 cancel order program
cd sp1-programs/cancel-order/program
cargo prove build

# Test cancellation script
cd ../script
ORDER_NOTE=../../create-order/script/order_note.json cargo run --release

# Generate proof
ORDER_NOTE=../../create-order/script/order_note.json GENERATE_PROOF=true cargo run --release

# Run Foundry tests
cd ../../../contracts
forge test --match-contract CancelOrderTest -vvv
```

## 7.9 Checklist

- [ ] SP1 cancel order program compiles
- [ ] Order nullifier correctly derived
- [ ] Order commitment verified
- [ ] Contract marks order as cancelled
- [ ] Contract rejects double cancellation
- [ ] Event emitted correctly
- [ ] TEE removes order from book
- [ ] Deposit unlocked for reuse
