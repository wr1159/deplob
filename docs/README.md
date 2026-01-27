# DePLOB Documentation

**DePLOB** (Decentralized Private Limit Order Book) - A privacy-preserving DEX using zk-SNARKs and TEE.

## Quick Reference

| Document | Description |
|----------|-------------|
| [01-problem-statement.md](./01-problem-statement.md) | Problem motivation, MEV issues, dark pool concept |
| [02-preliminary-knowledge.md](./02-preliminary-knowledge.md) | Blockchain, DeFi, ZKP, Merkle Trees, Nullifiers, TEE |
| [03-system-architecture.md](./03-system-architecture.md) | System components, data structures, security model |
| [04-operations.md](./04-operations.md) | Step-by-step: Deposit, Withdraw, Create/Cancel Order, Execution |

## Core Concepts

### What is DePLOB?
An on-chain shielded pool DEX where users swap tokens via limit orders while preserving privacy. Order data (sizing, direction, depositor) is encrypted and invisible on-chain.

### Key Technologies
- **zk-SNARKs**: Prove transaction validity without revealing details
- **TEE (Trusted Execution Environment)**: Secure order matching off-chain
- **Incremental Merkle Tree**: Efficient commitment storage
- **Nullifiers**: Prevent double-spending while maintaining privacy

### System Components
```
Smart Contract (Shielded Pool) <--> ZK Verifier <--> TEE Matching Engine
```

## Operation Flow

```
1. DEPOSIT    User -> Smart Contract: Commit tokens to shielded pool
2. CREATE     User -> Smart Contract -> TEE: Submit encrypted order
3. MATCH      TEE: Decrypt, match orders, generate settlement
4. SETTLE     TEE -> Smart Contract: Create new commitments for trades
5. WITHDRAW   User -> Smart Contract: Withdraw to any address (unlinkable)
```

## Key Data Structures

### Commitment
```
commitment = H(nullifier_note || secret || token || amount || order_data)
```

### Nullifier
```
nullifier = H(nullifier_note)  // Prevents double-spend, maintains privacy
```

### Order (Encrypted)
```
{price, quantity, side, token_in, token_out, commitment_reference}
```

## Privacy Guarantees

- Balance privacy via commitments
- Order privacy via encryption
- Identity privacy via unlinkable deposits/withdrawals
- Trade privacy via TEE execution

## Building With This Documentation

When coding DePLOB components, refer to:
- **Smart Contracts**: See architecture + operations for interfaces
- **ZK Circuits**: See preliminary knowledge + operations for proof relations
- **TEE Engine**: See architecture + operations for matching logic
- **Frontend**: See operations for user-side steps
