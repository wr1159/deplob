# 06 — Order Creation

## Overview

Users submit limit orders directly to the TEE's HTTP API. The TEE verifies deposit
ownership internally and adds the order to the in-memory matching engine. No smart
contract interaction is required for order creation — only `settleMatch()` hits
the chain when a trade occurs.

---

## Flow

```
User
  │
  ├── (has deposit_note.json from step 04)
  │
  └── POST https://<tee-host>/v1/orders
        body: deposit secrets + Merkle proof + order params
              │
              ▼
           TEE Server (tee/src/routes/orders.rs)
              │
              ├── Recompute commitment from secrets
              ├── Verify Merkle proof (deposit is in tree)
              ├── eth_call: commitment known on-chain?
              ├── eth_call: nullifier not spent?
              ├── eth_call: Merkle root valid?
              ├── Verify deposit covers order amount
              ├── Lock deposit in locked_deposits
              ├── Add to MatchingEngine
              └── Run matching loop
                    │
                    └── (on match) → settleMatch() on-chain
```

No ZK proof is needed. The TEE is the trusted verifier — it recomputes the
commitment from the user's secrets and checks on-chain state directly.

---

## API Endpoint

### `POST /v1/orders`

**Request body:**

```json
{
  "deposit_nullifier_note": "0x<32 bytes hex>",
  "deposit_secret":         "0x<32 bytes hex>",
  "deposit_token":          "0x<20 bytes hex>",
  "deposit_amount":         "1000000000000000000",
  "merkle_root":            "0x<32 bytes hex>",
  "merkle_siblings":        ["0x<32 bytes>", ...],
  "merkle_path_indices":    [0, 1, 0, ...],
  "order": {
    "price":    "1000",
    "quantity": "500000000000000000",
    "side":     "sell",
    "token_in": "0x<20 bytes hex>",
    "token_out": "0x<20 bytes hex>"
  }
}
```

**Fields:**

| Field | Description |
|-------|-------------|
| `deposit_nullifier_note` | 32-byte random secret from `deposit_note.json` |
| `deposit_secret` | 32-byte random secret from `deposit_note.json` |
| `deposit_token` | Token address deposited (must match `order.token_in`) |
| `deposit_amount` | Amount deposited (decimal string) |
| `merkle_root` | A recent Merkle root from the contract (from `getLastRoot()`) |
| `merkle_siblings` | 20 sibling hashes from deposit leaf to root |
| `merkle_path_indices` | 20 path indices (0 = left child, 1 = right child) |
| `order.price` | Price in base units (token_out per token_in) |
| `order.quantity` | Quantity of token_in being offered |
| `order.side` | `"buy"` or `"sell"` |
| `order.token_in` | Token the user is offering |
| `order.token_out` | Token the user wants to receive |

**Success response (200):**

```json
{
  "order_id": "0x<32 bytes hex>",
  "status": "accepted"
}
```

Save `order_id` — it is needed to cancel the order later.

**Error responses:**

| Status | Meaning |
|--------|---------|
| 400 | Invalid input, bad Merkle proof, insufficient deposit |
| 409 | Deposit already backing another open order |
| 500 | Chain query failure |

---

## TEE Verification Steps

The handler in `tee/src/routes/orders.rs` performs these checks under a single
write lock to prevent races:

1. Parse and decode all hex fields
2. Call `verify_deposit_ownership()` (`tee/src/verification.rs`):
   - Recompute `commitment = keccak256(nullifier_note || secret || token || amount)`
   - Recompute `deposit_nullifier = keccak256(nullifier_note)`
   - Verify Merkle proof: `MerkleProof.verify(&commitment, &merkle_root)`
3. Call `verify_deposit_covers_order()`:
   - Sell: `deposit_token == token_in` and `deposit_amount >= quantity`
   - Buy:  `deposit_token == token_in` and `deposit_amount >= quantity * price`
4. Check `locked_deposits[deposit_nullifier]` is empty (not already used)
5. `chain.is_commitment_known(commitment)` must return `true`
6. `chain.is_nullifier_spent(deposit_nullifier)` must return `false`
7. `chain.is_known_root(merkle_root)` must return `true`
8. Compute `order_id = keccak256(deposit_nullifier || price || quantity || side || token_in || token_out)`
9. Insert into `locked_deposits`, `order_to_deposit`, `order_details`
10. Call `add_and_match()` — runs matching loop, returns any trades
11. For each trade: call `generate_settlement()` then `chain.settle_match()`

---

## Order ID

```
order_id = keccak256(
    deposit_nullifier  (32 bytes)
    price              (u128 as 32-byte big-endian)
    quantity           (u128 as 32-byte big-endian)
    side               (0x00=buy / 0x01=sell, padded to 32 bytes)
    token_in           (20-byte address, right-padded to 32)
    token_out          (20-byte address, right-padded to 32)
)
```

This ties each order uniquely to a deposit and its parameters.

---

## Double-Spend Protection

The on-chain `orderNullifiers` mapping has been removed. Protection is now
enforced in the TEE's in-memory `locked_deposits: HashMap<[u8;32], [u8;32]>`:

- Key: `deposit_nullifier`
- Value: `order_id` currently backed by this deposit

A deposit can back at most one open order at a time. The TEE checks and inserts
atomically under a write lock. On settlement or cancellation, the entry is
removed, freeing the deposit for a new order or withdrawal.

---

## Client Example (TypeScript)

```typescript
async function submitOrder(
  depositNote: DepositNote,
  merkleProof: MerkleProof,
  order: { price: bigint; quantity: bigint; side: 'buy' | 'sell';
           tokenIn: string; tokenOut: string },
  teeUrl: string,
) {
  const response = await fetch(`${teeUrl}/v1/orders`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      deposit_nullifier_note: toHex(depositNote.nullifierNote),
      deposit_secret:         toHex(depositNote.secret),
      deposit_token:          depositNote.token,
      deposit_amount:         depositNote.amount.toString(),
      merkle_root:            toHex(merkleProof.root),
      merkle_siblings:        merkleProof.siblings.map(toHex),
      merkle_path_indices:    merkleProof.pathIndices,
      order: {
        price:     order.price.toString(),
        quantity:  order.quantity.toString(),
        side:      order.side,
        token_in:  order.tokenIn,
        token_out: order.tokenOut,
      },
    }),
  });

  if (!response.ok) {
    const { error } = await response.json();
    throw new Error(error);
  }

  const { order_id } = await response.json();
  return order_id; // save for cancellation
}
```

---

## Key Source Files

| File | Purpose |
|------|---------|
| `tee/src/routes/orders.rs` | `submit_order` HTTP handler |
| `tee/src/verification.rs` | `verify_deposit_ownership`, `verify_deposit_covers_order` |
| `tee/src/matching/mod.rs` | `add_and_match` — runs matching loop |
| `tee/src/settlement/mod.rs` | `generate_settlement` — creates new deposit notes |
| `tee/src/state.rs` | `TeeState`, `locked_deposits` |

---

## Verification Checklist

- [ ] Deposit secrets rejected if commitment not on-chain
- [ ] Deposit secrets rejected if nullifier already spent
- [ ] Deposit rejected if amount insufficient for order (sell: < quantity; buy: < quantity × price)
- [ ] Same deposit cannot back two concurrent orders (409 on second request)
- [ ] Matching triggers settlement when crossing orders exist
- [ ] After settlement, `locked_deposits` entries are cleared
