# 07 — Order Cancellation

## Overview

Users cancel open orders by sending their deposit `nullifier_note` to the TEE.
The TEE derives the `deposit_nullifier`, looks up the associated order, removes
it from the matching engine, and frees the deposit. No smart contract call is
needed — the chain only touches state during `settleMatch()`.

---

## Flow

```
User
  │
  ├── (has deposit_note.json — same secrets used to create the order)
  │
  └── DELETE https://<tee-host>/v1/orders/<order_id>
        body: { "deposit_nullifier_note": "0x..." }
              │
              ▼
           TEE Server (tee/src/routes/orders.rs::cancel_order)
              │
              ├── Derive deposit_nullifier = keccak256(deposit_nullifier_note)
              ├── Lookup locked_deposits[deposit_nullifier] == order_id?
              ├── Remove order from MatchingEngine
              └── Free deposit from locked_deposits
                    │
                    └── Return { order_id, status: "cancelled", deposit_nullifier }
```

No ZK proof required. The user proves ownership of the deposit by knowing the
`nullifier_note`. The TEE verifies by recomputing `deposit_nullifier` and
checking it is the key that maps to the requested `order_id`.

---

## API Endpoint

### `DELETE /v1/orders/:order_id`

**Path parameter:** `order_id` — hex-encoded 32-byte order id returned by `POST /v1/orders`.

**Request body:**

```json
{
  "deposit_nullifier_note": "0x<32 bytes hex>"
}
```

**Success response (200):**

```json
{
  "order_id": "0x<32 bytes hex>",
  "status": "cancelled",
  "deposit_nullifier": "0x<32 bytes hex>"
}
```

The returned `deposit_nullifier` is now unlocked and the deposit can be:
- Used to create a new order (`POST /v1/orders`)
- Withdrawn from the shielded pool (`withdraw()`)

**Error responses:**

| Status | Meaning |
|--------|---------|
| 400 | Malformed `order_id` or `deposit_nullifier_note` |
| 403 | The `deposit_nullifier_note` does not own the specified `order_id` |
| 404 | No open order found for this deposit |

---

## Authentication Model

The user authenticates by knowing `deposit_nullifier_note`, from which the TEE
derives:

```
deposit_nullifier = keccak256(deposit_nullifier_note)
```

The TEE checks that `locked_deposits[deposit_nullifier] == order_id`. If the
mapping exists and matches, the user is proven to be the order owner. An
attacker who doesn't know `nullifier_note` cannot derive the correct
`deposit_nullifier`, so they cannot cancel someone else's order.

This is secure because:
1. The TEE runs in a hardware-isolated enclave (memory never visible to host OS)
2. Communication is over in-enclave TLS (production: attested TLS)
3. `nullifier_note` is 32 random bytes — brute-force infeasible

---

## Effect on Deposit State

After cancellation:
- `locked_deposits[deposit_nullifier]` is removed
- `order_to_deposit[order_id]` is removed
- `order_details[order_id]` is removed
- The order is removed from `OrderBook` via `remove_order(&order_id)`

The deposit remains in the on-chain Merkle tree unchanged. Its nullifier has not
been spent, so the user can either create a new order or withdraw the deposit
at any time.

---

## Client Example (TypeScript)

```typescript
async function cancelOrder(
  orderId: string,
  depositNote: DepositNote,
  teeUrl: string,
) {
  const response = await fetch(`${teeUrl}/v1/orders/${orderId}`, {
    method: 'DELETE',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      deposit_nullifier_note: toHex(depositNote.nullifierNote),
    }),
  });

  if (!response.ok) {
    const { error } = await response.json();
    throw new Error(error);
  }

  return await response.json(); // { order_id, status, deposit_nullifier }
}
```

---

## Key Source Files

| File | Purpose |
|------|---------|
| `tee/src/routes/orders.rs` | `cancel_order` HTTP handler |
| `tee/src/orderbook/mod.rs` | `OrderBook::remove_order` |
| `tee/src/state.rs` | `locked_deposits`, `order_to_deposit` cleanup |

---

## Verification Checklist

- [ ] Cancellation with correct `deposit_nullifier_note` succeeds
- [ ] Cancellation with wrong `deposit_nullifier_note` returns 403
- [ ] Cancellation of non-existent order returns 404
- [ ] After cancellation, deposit is no longer in `locked_deposits`
- [ ] After cancellation, deposit can be used for a new order
- [ ] After cancellation, order no longer appears in matching engine
