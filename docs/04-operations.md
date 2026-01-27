# DePLOB: Operations Guide

This document details the step-by-step processes for all core operations in DePLOB.

---

## 1. Deposit

Deposit tokens into the shielded pool, creating a commitment that hides the depositor's identity.

### User Steps

1. **Generate Random Values**
   ```
   nullifier_note = random_field_element()
   secret = random_field_element()
   ```

2. **Create Commitment**
   ```
   commitment = H(nullifier_note || secret || token_address || amount)
   ```

3. **Generate ZK Proof**
   - Public inputs: `commitment`, `token_address`, `amount`
   - Private inputs: `nullifier_note`, `secret`
   - Proves: commitment is correctly formed

4. **Submit Transaction**
   ```solidity
   deposit(commitment, token_address, amount, proof)
   ```

### Smart Contract Steps

1. **Verify ZK Proof**
   ```solidity
   require(verifier.verifyDeposit(proof, commitment, token, amount));
   ```

2. **Transfer Tokens**
   ```solidity
   IERC20(token).transferFrom(msg.sender, address(this), amount);
   ```

3. **Add Commitment to Merkle Tree**
   ```solidity
   commitments[nextLeafIndex] = commitment;
   merkleRoot = updateMerkleRoot(commitment, nextLeafIndex);
   nextLeafIndex++;
   ```

4. **Emit Event**
   ```solidity
   emit Deposit(commitment, leafIndex, timestamp);
   ```

### Data Stored

| Location | Data |
|----------|------|
| User (off-chain) | `nullifier_note`, `secret`, `leaf_index` |
| Smart Contract | `commitment` in Merkle tree |

---

## 2. Withdraw

Withdraw tokens from the shielded pool to any address without revealing the original deposit.

### User Steps

1. **Compute Nullifier**
   ```
   nullifier = H(nullifier_note)
   ```

2. **Generate Merkle Proof**
   - Obtain sibling hashes for path from leaf to root

3. **Generate ZK Proof**
   - Public inputs: `nullifier`, `merkle_root`, `recipient`, `token`, `amount`
   - Private inputs: `nullifier_note`, `secret`, `merkle_path`, `leaf_index`
   - Proves:
     - Knowledge of `(nullifier_note, secret)` such that `H(nullifier_note || secret || token || amount)` exists in tree
     - `nullifier = H(nullifier_note)`

4. **Submit Transaction** (can use relayer for privacy)
   ```solidity
   withdraw(nullifier, recipient, token, amount, merkle_root, proof)
   ```

### Smart Contract Steps

1. **Check Nullifier Not Spent**
   ```solidity
   require(!spentNullifiers[nullifier], "Already spent");
   ```

2. **Verify Merkle Root is Valid**
   ```solidity
   require(isKnownRoot(merkle_root), "Invalid root");
   ```

3. **Verify ZK Proof**
   ```solidity
   require(verifier.verifyWithdraw(proof, nullifier, merkle_root, recipient, token, amount));
   ```

4. **Mark Nullifier as Spent**
   ```solidity
   spentNullifiers[nullifier] = true;
   ```

5. **Transfer Tokens**
   ```solidity
   IERC20(token).transfer(recipient, amount);
   ```

6. **Emit Event**
   ```solidity
   emit Withdrawal(nullifier, recipient, token, amount);
   ```

---

## 3. Create Order

Submit an encrypted limit order that only the TEE can decrypt.

### User Steps

1. **Prepare Order Data**
   ```
   order = {
       price: limit_price,
       quantity: order_quantity,
       side: BUY | SELL,
       token_in: token_selling,
       token_out: token_buying
   }
   ```

2. **Generate Order Commitment**
   ```
   order_nullifier_note = random_field_element()
   order_secret = random_field_element()
   order_commitment = H(order_nullifier_note || order_secret || order_data)
   ```

3. **Link to Deposit Commitment**
   - Reference existing deposit that covers order amount

4. **Encrypt Order for TEE**
   ```
   encrypted_order = encrypt(TEE_public_key, order || deposit_reference)
   ```

5. **Generate ZK Proof**
   - Public inputs: `order_commitment`, `deposit_nullifier`
   - Private inputs: `order_data`, `deposit_secret`, `deposit_nullifier_note`, `merkle_path`
   - Proves:
     - User owns deposit with sufficient balance
     - Order commitment correctly formed
     - Deposit not already used

6. **Submit Transaction**
   ```solidity
   createOrder(encrypted_order, order_commitment, deposit_nullifier, proof)
   ```

### Smart Contract Steps

1. **Check Deposit Nullifier Valid**
   ```solidity
   require(!orderNullifiers[deposit_nullifier], "Deposit already used for order");
   ```

2. **Verify ZK Proof**
   ```solidity
   require(verifier.verifyCreateOrder(proof, order_commitment, deposit_nullifier));
   ```

3. **Mark Deposit as Locked**
   ```solidity
   orderNullifiers[deposit_nullifier] = true;
   ```

4. **Forward to TEE**
   ```solidity
   emit OrderCreated(encrypted_order, order_commitment, timestamp);
   ```

### TEE Steps

1. **Decrypt Order**
   ```
   order = decrypt(TEE_private_key, encrypted_order)
   ```

2. **Validate Order**
   - Check price > 0
   - Check quantity > 0
   - Verify deposit reference

3. **Add to Order Book**
   ```
   if order.side == BUY:
       insert into bid_book ordered by (price DESC, timestamp ASC)
   else:
       insert into ask_book ordered by (price ASC, timestamp ASC)
   ```

4. **Attempt Matching** (see Execution section)

---

## 4. Cancel Order

Cancel an existing order and unlock the associated deposit.

### User Steps

1. **Compute Order Nullifier**
   ```
   order_nullifier = H(order_nullifier_note)
   ```

2. **Generate ZK Proof**
   - Public inputs: `order_nullifier`, `order_commitment`
   - Private inputs: `order_nullifier_note`, `order_secret`, `order_data`
   - Proves:
     - Knowledge of order preimage
     - Order commitment matches

3. **Submit Transaction**
   ```solidity
   cancelOrder(order_nullifier, order_commitment, proof)
   ```

### Smart Contract Steps

1. **Check Order Nullifier Not Used**
   ```solidity
   require(!cancelledOrders[order_nullifier], "Already cancelled");
   ```

2. **Verify ZK Proof**
   ```solidity
   require(verifier.verifyCancelOrder(proof, order_nullifier, order_commitment));
   ```

3. **Mark as Cancelled**
   ```solidity
   cancelledOrders[order_nullifier] = true;
   ```

4. **Emit Event**
   ```solidity
   emit OrderCancelled(order_nullifier, order_commitment);
   ```

### TEE Steps

1. **Receive Cancellation Event**

2. **Find Order in Book**
   ```
   order = find_by_commitment(order_commitment)
   ```

3. **Remove from Order Book**
   ```
   remove from bid_book or ask_book
   ```

4. **Unlock Deposit**
   - Generate proof that deposit can be withdrawn again
   - Or: Create new commitment for unlocked funds

---

## 5. Order Execution (Matching)

TEE matches orders and generates settlement proofs.

### TEE Matching Process

1. **Check for Crossing Orders**
   ```
   best_bid = max(bid_book.price)
   best_ask = min(ask_book.price)

   while best_bid >= best_ask:
       execute_match()
   ```

2. **Execute Match**
   ```
   bid_order = bid_book.top()
   ask_order = ask_book.top()

   execution_price = ask_order.price  // Price of resting order
   execution_quantity = min(bid_order.quantity, ask_order.quantity)

   // Update quantities
   bid_order.quantity -= execution_quantity
   ask_order.quantity -= execution_quantity

   // Remove filled orders
   if bid_order.quantity == 0: remove bid_order
   if ask_order.quantity == 0: remove ask_order
   ```

3. **Generate Settlement Commitments**
   ```
   // For buyer: receives token_out
   buyer_new_commitment = H(new_nullifier || new_secret || token_out || execution_quantity)

   // For seller: receives token_in
   seller_new_commitment = H(new_nullifier || new_secret || token_in || execution_value)
   ```

4. **Generate Settlement Proof**
   - Proves correct matching
   - Proves correct commitment generation
   - Signs with TEE attestation

5. **Submit Settlement to Smart Contract**
   ```solidity
   settleMatch(
       buyer_old_nullifier,
       seller_old_nullifier,
       buyer_new_commitment,
       seller_new_commitment,
       tee_attestation,
       settlement_proof
   )
   ```

### Smart Contract Settlement

1. **Verify TEE Attestation**
   ```solidity
   require(verifyAttestation(tee_attestation), "Invalid TEE");
   ```

2. **Verify Settlement Proof**
   ```solidity
   require(verifier.verifySettlement(settlement_proof, ...));
   ```

3. **Spend Old Commitments**
   ```solidity
   spentNullifiers[buyer_old_nullifier] = true;
   spentNullifiers[seller_old_nullifier] = true;
   ```

4. **Create New Commitments**
   ```solidity
   addCommitment(buyer_new_commitment);
   addCommitment(seller_new_commitment);
   ```

5. **Emit Event**
   ```solidity
   emit TradeSettled(buyer_new_commitment, seller_new_commitment, timestamp);
   ```

### Partial Fills

When orders partially fill:

1. **Reduce Order Quantity**
   - Original order remains in book with reduced quantity

2. **Create Partial Commitment**
   - New commitment for filled portion
   - Original commitment still valid for remainder

3. **Update Order Reference**
   - Link remaining order to updated commitment

---

## Operation Summary

| Operation | On-Chain Gas | Privacy Level | TEE Involvement |
|-----------|--------------|---------------|-----------------|
| Deposit | Medium | Commitment hidden | None |
| Withdraw | Medium | Full unlinkability | None |
| Create Order | Low | Order encrypted | Decrypt & store |
| Cancel Order | Low | Order details hidden | Remove from book |
| Execution | High | Trade details hidden | Match & settle |

---

## ZK Circuit Summary

| Circuit | Public Inputs | Private Inputs | Purpose |
|---------|---------------|----------------|---------|
| Deposit | commitment, token, amount | nullifier_note, secret | Valid commitment |
| Withdraw | nullifier, root, recipient, token, amount | nullifier_note, secret, merkle_path | Ownership proof |
| CreateOrder | order_commitment, deposit_nullifier | order_data, deposit_secrets, merkle_path | Order validity |
| CancelOrder | order_nullifier, order_commitment | order_nullifier_note, order_secret | Cancellation auth |
| Settlement | nullifiers, new_commitments | trade_details, old_secrets | Trade validity |
