# 09 — Testing

## Overview

Testing is organized into three layers:

1. **Foundry unit + integration tests** — smart contract behaviour
2. **Rust unit tests** — deplob-core primitives, TEE order book + matching
3. **TEE route integration tests** — HTTP handler logic (axum in-process)

---

## 9.1 Testing Structure

```
contracts/test/
├── MerkleTree.t.sol      - 9 tests
├── DePLOB.t.sol          - 18 tests (deposit, withdraw, settle, admin, views)
├── DepositE2E.t.sol      - 4 FFI tests
├── WithdrawE2E.t.sol     - 5 FFI tests
└── mocks/

sp1-programs/lib/deplob-core/
└── src/{merkle,commitment,keccak}.rs  - inline unit tests

tee/src/
├── orderbook/mod.rs      - 5 unit tests
└── matching/mod.rs       - 6 unit tests
```

---

## 9.2 Foundry Tests

Run all 36 tests:

```bash
cd contracts
forge test
forge test -vv          # verbose (shows logs + events)
forge test --gas-report
forge coverage
```

### What is tested

**Deposit:** success, duplicate commitment, unsupported token, zero amount.

**Withdrawal:** success, double-spend prevention, unknown root.

**Settlement:** TEE-only guard, success + nullifiers marked spent, replay prevention.

**Admin:** `addSupportedToken`, `removeSupportedToken`, `setTEEOperator`, `transferOwnership`.

**Views:** `isKnownRoot`, `isSpentNullifier`.

**Note:** `createOrder` and `cancelOrder` no longer exist on the contract. The 6
tests that covered those functions have been removed. Order lifecycle is now
entirely managed by the TEE and tested at the Rust level (see §9.4).

### Key test patterns

```solidity
// DePLOBTest constructor (matches actual contract)
deplob = new DePLOB(address(verifier), WITHDRAW_VKEY, teeOperator);

// TEE-only guard pattern
vm.prank(alice);
vm.expectRevert("Not TEE operator");
deplob.settleMatch(...);

// Settlement emits TradeSettled + marks nullifiers spent
vm.prank(teeOperator);
deplob.settleMatch(buyerNullifier, sellerNullifier, buyerNew, sellerNew, "", "");
assertTrue(deplob.isSpentNullifier(buyerNullifier));
assertTrue(deplob.isSpentNullifier(sellerNullifier));
```

---

## 9.3 Merkle Tree Tests (`MerkleTree.t.sol`)

9 tests covering:

- Initial state and root validity
- Root changes after insert
- Multiple insertions, all roots in history
- Root history eviction (>30 roots)
- Fuzz test: `testFuzz_InsertEmitsEvent(bytes32 leaf)`
- Hash pair order (left vs right sibling)
- Zero hash consistency

---

## 9.4 Rust Unit Tests

### deplob-core

```bash
cargo test -p deplob-core
```

Tests in `commitment.rs`: commitment determinism, nullifier ≠ commitment,
different inputs → different outputs, type conversions.

Tests in `merkle.rs` (run existing tests):

```bash
cargo test -p deplob-core -- merkle
```

### TEE Order Book (`tee/src/orderbook/mod.rs`)

```bash
cargo test -p deplob-tee -- orderbook
```

| Test | Verifies |
|------|---------|
| `test_best_bid_is_highest_price` | BTreeMap reverse iteration |
| `test_best_ask_is_lowest_price` | BTreeMap forward iteration |
| `test_crossing_detected` | `has_crossing()` when bid >= ask |
| `test_no_crossing` | `has_crossing()` returns false when bid < ask |
| `test_remove_order_by_id` | `remove_order` clears book + index |
| `test_time_priority_fifo` | Same-price orders: earlier one pops first |

### TEE Matching Engine (`tee/src/matching/mod.rs`)

```bash
cargo test -p deplob-tee -- matching
```

| Test | Verifies |
|------|---------|
| `test_exact_match` | Full fill, both sides cleared |
| `test_no_match_price_gap` | No trade when bid < ask |
| `test_partial_fill_bid_larger` | Bid remainder reinserted |
| `test_partial_fill_ask_larger` | Ask remainder reinserted |
| `test_price_priority_asks` | Lowest ask matched first |
| `test_multiple_trades_one_bid_many_asks` | One bid consumes two asks |

Run all TEE tests:

```bash
cargo test -p deplob-tee
```

---

## 9.5 TEE Route Integration Tests (Recommended additions)

These test the HTTP handlers in-process using `axum`'s test utilities, with a
`MockChainClient` that pre-populates known commitments.

```rust
// tee/tests/integration.rs (example structure)
use axum::http::StatusCode;
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn test_submit_order_valid() {
    let chain = Arc::new(MockChainClient { /* pre-loaded commitment */ });
    let app = routes::router(new_shared(chain));

    let body = /* valid OrderRequest JSON */;
    let response = app
        .oneshot(Request::builder().method("POST").uri("/v1/orders")
            .header("content-type", "application/json")
            .body(Body::from(body)).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

Recommended tests:

| Test | Expected status |
|------|----------------|
| Valid order, all checks pass | 200 |
| Same deposit used twice | 409 |
| Bad Merkle proof | 400 |
| Insufficient deposit (sell > amount) | 400 |
| Cancel with correct nullifier_note | 200 |
| Cancel with wrong nullifier_note | 403 |
| Cancel non-existent order_id | 404 |
| Cross order: buy + sell → trade | 200 (both) + settlement called |

---

## 9.6 SP1 Tests

```bash
# deplob-core unit tests (keccak, commitment, merkle)
cargo test -p deplob-core

# Withdraw script FFI test data generator
cargo run --release --bin generate_withdraw_test_data \
  --manifest-path sp1-programs/withdraw/script/Cargo.toml
```

SP1 create-order and cancel-order program tests are not applicable — those programs
do not exist in the new architecture (order creation/cancellation has no SP1 proof).

---

## 9.7 Invariant Tests (Foundry)

`contracts/test/Invariant.t.sol` — fuzz-based invariants:

```bash
forge test --match-contract InvariantTest --fuzz-runs 10000
```

Key invariants:
- `invariant_SolvencyCheck`: contract token balance >= totalDeposited - totalWithdrawn
- `invariant_RootAlwaysValid`: `getLastRoot()` is always a known root

---

## 9.8 Run All Tests

```bash
# Smart contract tests (36 tests)
cd contracts && forge test

# Rust workspace tests (12 TEE + deplob-core)
cd .. && cargo test

# Specific packages
cargo test -p deplob-core
cargo test -p deplob-tee
```

---

## 9.9 Checklist

- [ ] All 36 Foundry tests pass
- [ ] All 12 TEE unit tests pass
- [ ] deplob-core tests pass
- [ ] TEE route integration tests (§9.5) written and passing
- [ ] Invariant tests run with 10k+ fuzz iterations
- [ ] Code coverage > 80% on contract
