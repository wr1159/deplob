# DePLOB: System Architecture

## High-Level Overview

DePLOB consists of three main modules that work together to ensure user balances remain private while order execution remains verifiable:

```
+-------------------+     +-------------------+     +-------------------+
|                   |     |                   |     |                   |
|  Smart Contract   |<--->|   ZK Circuit      |<--->|  TEE Matching     |
|  (Shielded Pool)  |     |   Verifier        |     |  Engine           |
|                   |     |                   |     |                   |
+-------------------+     +-------------------+     +-------------------+
        ^                                                    |
        |                                                    |
        v                                                    v
+-------------------+                              +-------------------+
|                   |                              |                   |
|  User Wallet      |                              |  Order Book       |
|  (Frontend)       |                              |  State            |
|                   |                              |                   |
+-------------------+                              +-------------------+
```

## Core Components

### 1. Smart Contract Layer (Shielded Pool)

The smart contract deployed on-chain manages:

**State:**
- Incremental Merkle Tree storing all commitments
- Set of spent nullifiers
- Token balances in the pool

**Functions:**
- `deposit()`: Add tokens to shielded pool
- `withdraw()`: Remove tokens from shielded pool
- `createOrder()`: Submit encrypted limit order
- `cancelOrder()`: Cancel existing order

**Data Structures:**
```solidity
// Incremental Merkle Tree for commitments
mapping(uint256 => bytes32) public commitments;
bytes32 public merkleRoot;
uint256 public nextLeafIndex;

// Nullifier tracking (prevents double-spend)
mapping(bytes32 => bool) public spentNullifiers;

// Token balances
mapping(address => mapping(address => uint256)) public balances;
```

### 2. Zero-Knowledge Circuit Verifier

On-chain verifier contract that validates ZK proofs for:
- Deposit commitments
- Withdrawal authorizations
- Order creation proofs
- Order cancellation proofs

**Proof Relations:**
- Balance consistency: User has sufficient funds
- Commitment validity: Correct commitment structure
- Merkle membership: Commitment exists in tree
- Nullifier correctness: Proper nullifier derivation

### 3. TEE-Based Matching Engine

Off-chain component running in Trusted Execution Environment:

**Responsibilities:**
- Decrypt encrypted orders
- Maintain private order book state
- Execute order matching (price-time priority)
- Generate settlement proofs
- Sign attestations for on-chain verification

**Order Book State (Private):**
```
Bid Book: [(price, quantity, commitment_id, timestamp), ...]  // Descending by price
Ask Book: [(price, quantity, commitment_id, timestamp), ...]  // Ascending by price
```

## Data Structures

### Commitment Structure

```
commitment = H(nullifier_note || secret || token || amount || order_data)
```

Where:
- `nullifier_note`: Random value for nullifier derivation
- `secret`: Random value for commitment hiding
- `token`: Token address
- `amount`: Token amount
- `order_data`: Optional order parameters (price, side, etc.)

### Order Structure

```
order = {
    commitment_id: bytes32,      // Reference to deposit commitment
    price: uint256,              // Limit price
    quantity: uint256,           // Order quantity
    side: enum { BUY, SELL },    // Order direction
    token_in: address,           // Token being sold
    token_out: address,          // Token being bought
    timestamp: uint256           // Order creation time
}
```

### Encrypted Order (On-Chain)

```
encrypted_order = {
    ciphertext: bytes,           // Order encrypted with TEE public key
    commitment_proof: bytes,     // ZK proof of valid commitment
    nullifier: bytes32           // For order uniqueness
}
```

## Security Model

### Privacy Guarantees

1. **Balance Privacy**: User balances hidden via commitments
2. **Order Privacy**: Order details encrypted until matched
3. **Identity Privacy**: Unlinkability between deposit and withdrawal addresses
4. **Trade Privacy**: Trade execution details remain confidential

### Trust Assumptions

1. **ZK Proofs**: Cryptographically sound (no trusted setup compromise)
2. **TEE Integrity**: Hardware attestation valid
3. **Smart Contract**: Correctly implemented and audited
4. **Merkle Tree**: Collision-resistant hash function

### Attack Mitigations

| Attack | Mitigation |
|--------|------------|
| Front-running | Orders encrypted until TEE decrypts |
| Double-spend | Nullifier tracking |
| Order replay | Unique nullifiers per order |
| TEE compromise | Remote attestation verification |
| Balance manipulation | ZK proofs for all state changes |

## Interaction Flow Summary

```
User                    Smart Contract              TEE Engine
  |                           |                          |
  |-- deposit(proof) -------->|                          |
  |                           |-- store commitment ----->|
  |                           |                          |
  |-- createOrder(enc) ------>|                          |
  |                           |-- forward encrypted ---->|
  |                           |                     decrypt & add to book
  |                           |                          |
  |                           |<---- settlement proof ---|
  |                           |                          |
  |-- withdraw(proof) ------->|                          |
  |<-- tokens ----------------|                          |
```

## Multi-Token Support

DePLOB supports trading across multiple token pairs:

1. **Unified Shielded Pool**: All tokens in single pool
2. **Cross-Pair Routing**: TEE can route orders across pairs
3. **Efficient Capital**: No need for separate deposits per pair

```
Pool State:
+------------------+
| ETH Commitments  |
| USDC Commitments |
| DAI Commitments  |
| ...              |
+------------------+
         |
         v
+------------------+
| TEE Matching     |
| ETH/USDC Book    |
| ETH/DAI Book     |
| USDC/DAI Book    |
+------------------+
```
