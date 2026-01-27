# DePLOB: Problem Statement

## Overview

DePLOB (Decentralized Private Limit Order Book) is an on-chain shielded pool DEX where users can swap tokens using limit orders while preserving privacy. Although users make transactions on the blockchain to create or cancel orders, the data is encrypted such that no order-related information (sizing, direction, depositor) is visible on the blockchain.

## Problem Motivation

### Transparency Issues in Blockchain

Every transaction on a blockchain (liquidity pool swaps, order placements, cancellations) results in a publicly visible state transition. This means:
- The entire order book state is visible
- All state updates are public
- User wallet addresses are exposed
- Transaction amounts are visible

### MEV and Front-Running

The public nature of blockchain transactions enables adversaries to:
- Observe pending transactions
- Infer trader intentions or liquidation thresholds
- Execute front-running attacks
- Perform sandwich attacks
- Conduct coordinated liquidation hunts

Bots routinely monitor broadcast transactions and compete via priority-gas-auctions (PGA) to insert themselves ahead of honest traders, extracting Miner Extractable Value (MEV) and degrading market fairness.

### The Need for Dark Pools

Dark pools in traditional finance are trading systems that:
- Do not publicly display orders
- Enable trades with reduced market impact
- Reduce information leakage
- Operate with limited transparency

Research shows that adding a dark pool to existing exchanges concentrates price-relevant information and improves price discovery. An on-chain dark pool DEX would benefit the DeFi ecosystem.

## Recurring Limitations in Existing Solutions

### 1. Centralized Trust in Matching/Decryption Entities

Many existing systems depend on:
- Small sets of trusted MPC nodes
- Single decryption oracles
- Fixed participation sets (e.g., Rialto, BlockAuction)

These represent single points of failure where collusion or key compromise can bias outcomes or leak trade data.

### 2. Batch-Based Execution

Protocols like Rialto and COMMON rely on batch-based auctions:
- Multi-block latency
- No real-time execution
- Orders locked until next batch
- Discourages active trading strategies
- Reduces liquidity responsiveness

### 3. Lack of Multi-Token and Routing Support

Most dark-pool DEXs:
- Designed around single-asset pairs
- Maintain separate private pools for each market
- Prevent efficient liquidity routing
- Lead to poor capital utilization
- Require multiple deposits across isolated pools

## DePLOB Solution

DePLOB addresses these gaps by integrating:

1. **Shielded Pool System**: Powered by zk-SNARKs to anonymize user balances and transactions
2. **TEE-Based Matching Engine**: Secure order matching off-chain using Trusted Execution Environments
3. **Multi-Token Trading**: Routing and settling orders across multiple asset pairs while preserving privacy
4. **Decentralized TEE Network** (Reach Goal): Allow any participant to join a network of TEE nodes executing matching logic

## Research Objectives

Design and implement a Decentralized Private Limit Order Book (DePLOB), evaluating its effectiveness and potential impact on the DeFi ecosystem.
