# DePLOB: Preliminary Knowledge

## Blockchain Fundamentals

### Architecture Layers

**Consensus Layer**: Enables a distributed network of nodes to agree on a single sequence of transactions and state transitions.
- Proof-of-Work (PoW): Used in Bitcoin
- Proof-of-Stake (PoS): Used in Ethereum

**Execution Layer**: Responsible for processing transactions, executing smart-contract logic, and updating global state.
- Processes each transaction in ordered blocks
- Applies computation, state changes, event generation
- Handles gas accounting

### Gas

Gas is the cost paid in the network cryptocurrency (e.g., Ether on Ethereum). Validators receive gas for running computations - the more intensive the computation, the higher the gas required.

### Asymmetric Cryptographic Key Pairs

Every blockchain user controls:
- **Private Key**: Kept secret, used to sign transactions
- **Public Key/Address**: Shared publicly to receive funds

When performing a transaction:
1. User signs with private key to authorize transfer
2. Network verifies signature using public key
3. Address is pseudonymous (not tied to real-world identity)

---

## Decentralized Finance (DeFi)

DeFi refers to financial services built on blockchain where smart contracts replace traditional intermediaries and enable permissionless access.

### Trading Mechanisms

**1. Automated Market Maker (AMM) / Constant-Function Market Maker (CFMM)**
- Deterministic pricing rule in smart contract pools
- Example formulas: `xy = k` or `x^3 * y^3 = k`
- Anyone can create trading pairs without permission
- Simple and gas-efficient

**2. Central Limit Order Book (CLOB)**
- Participants submit buy (bid) and sell (ask) orders with price and quantity
- Orders stored in queue sorted by price
- Matching rules: price-time priority (best price first, then first-come-first-served)

### CLOB Data Structure

Each order is a tuple:
```
o_i = (p_i, q_i, s_i, t_i)
```
Where:
- `p_i`: Limit price
- `q_i`: Quantity
- `s_i`: Side (buy or sell)
- `t_i`: Timestamp

Order sets:
- **Bid Set B**: All buy orders, sorted descending by price
- **Ask Set A**: All sell orders, sorted ascending by price

### Trade Matching

A trade occurs when an incoming order `o_k` satisfies:
- For buy: `p_k >= min(A)` (buy price >= lowest ask)
- For sell: `p_k <= max(B)` (sell price <= highest bid)

Execution price `p*` is typically the resting order's price at top of book.

After execution: `q_i' = q_i - q_traded`
Orders with `q_i' = 0` are removed.

---

## Zero-Knowledge Proofs (ZKP)

A cryptographic protocol where a prover convinces a verifier that a statement is true without revealing any additional information.

### Formal Definition

Let R be a relation where `(x, w) in R` iff witness `w` satisfies statement `x`.

```
ZKProof: P(x, w) => V(x) such that (x, w) in R
```

### Key Properties

**1. Completeness**: If `(x, w) in R` and both parties are honest, verifier always accepts.
```
Pr[V accepts | (x, w) in R] = 1
```

**2. Soundness**: If `(x, w) not in R`, no polynomial-time prover can convince V except with negligible probability.
```
Pr[V accepts | (x, w) not in R] <= epsilon
```

**3. Zero-Knowledge**: Verifier learns nothing beyond validity. A simulator S can produce transcripts indistinguishable from real interactions.

### zk-SNARK

Zero-Knowledge Succinct Non-Interactive Argument of Knowledge:
- Prover generates single compact proof `pi`
- Any verifier can check using public verification key `vk`

```
Verify(vk, x, pi) = 1 if exists w: (x, w) in R, else 0
```

### Blockchain Applications

- Validate transactions without revealing private inputs
- Privacy-preserving cryptocurrencies (e.g., Zcash)
- Shielded pools and private exchanges

---

## Merkle Tree

A cryptographic data structure for efficiently verifying integrity and membership of large data sets.

### Structure

- Leaf nodes: `L_i = H(d_i)` (hash of data block)
- Internal nodes: `N_{i,j} = H(N_{i-1,2j-1} || N_{i-1,2j})`
- Merkle root: `R = H(N_{h,1} || N_{h,2})` where `h = log2(n)`

The root R uniquely commits to all data blocks.

### Merkle Proofs

To prove leaf `L_k` is in tree:
1. Provide sibling hashes `{S_1, S_2, ..., S_h}` along path to root
2. Verifier recomputes: `R' = H(...H(H(L_k || S_1) || S_2)...)`
3. Check `R' = R`

Proof requires O(log n) hashes.

### Incremental Merkle Tree (IMT)

Used in smart contracts:
- Fixed depth with all leaves initially zero
- New leaves added incrementally left-to-right
- Gas-efficient appends and on-chain verification
- Only small portion of tree updated per insertion

---

## Nullifiers

Cryptographic primitive ensuring uniqueness and preventing double-spending in privacy-preserving protocols.

### Commitment Creation

Users generate commitment from:
- **Nullifier note** `n`: Random parameter
- **Secret** `s`: Random parameter

```
C = H(n || s)
```

Commitment C is inserted as leaf in public Merkle tree T.

### Withdrawal Process

User constructs ZK proof attesting:
1. They know valid leaf C within T
2. Associated nullifier hasn't been revealed

Proof asserts knowledge of `(n, s)` such that:
```
H(n || s) in T  AND  Nullifier = H(n)
```

### Properties

- **Nullifier** `H(n)`: One-time pseudonym
- Detects if commitment already spent
- Maintains unlinkability between deposit and withdrawal
- Once nullifier recorded on-chain, reuse is rejected (single-spend constraint)

---

## Trusted Execution Environment (TEE)

Hardware-enforced secure area within a processor guaranteeing confidentiality and integrity of code and data.

### Key Features

- **Enclave**: Isolated execution context
- Protected from host OS, hypervisor, other applications
- Hardware primitives: memory encryption, secure paging, access control

### Remote Attestation

TEE produces cryptographically signed statement proving specific code executed in authentic enclave:
```
sigma = Sign_TEE(H(code) || H(data))
```

Can be verified on-chain to confirm correct, untampered execution.

### Blockchain Applications

- Off-chain execution of sensitive computations
- Decryption with private keys
- Private smart contracts
- Reduces need for complex multi-party cryptographic protocols

### Limitations

- Depends on hardware vendor trust chain
- Subject to side-channel attacks
- Speculative execution leaks
- Rollback vulnerabilities
- May need complementary cryptographic proofs for decentralized settings
