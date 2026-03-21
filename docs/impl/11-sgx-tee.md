# 11 — SGX TEE Attestation

## Overview

The TEE matching engine currently trusts the operator via `msg.sender == teeOperator`
(the `onlyTEE` modifier). This document specifies how to add cryptographic attestation
using Intel SGX so the smart contract can verify that settlement data was produced by
code running inside a genuine enclave.

The implementation is split into two phases:

- **Phase A — ECDSA Attestation:** The TEE signs settlement data with a secp256k1 key.
  The contract verifies the signature via `ecrecover`. No SGX hardware required.
- **Phase B — Gramine SGX Enclave:** The same binary runs inside a Gramine SGX enclave.
  The signing key is sealed to MRENCLAVE. The operator registers the enclave's signing
  address on-chain after verifying the MRENCLAVE out-of-band.

---

## 11.1 Attestation Data Format

The TEE signs a hash of the settlement parameters:

```
settlement_hash = keccak256(abi.encodePacked(
    buyerOldNullifier,
    sellerOldNullifier,
    buyerNewCommitment,
    sellerNewCommitment
))
```

The `attestation` bytes passed to `settleMatch` are a 65-byte ECDSA signature
(r ‖ s ‖ v) over the EIP-191 prefixed hash:

```
eth_signed_hash = keccak256("\x19Ethereum Signed Message:\n32" || settlement_hash)
attestation     = ECDSA_sign(eth_signed_hash, enclave_private_key)
```

EIP-191 prefixing ensures compatibility between alloy's `sign_hash` and Solidity's
`ecrecover`.

---

## 11.2 Rust — Attestation Module

New file: `tee/src/attestation.rs`

### Trait

```rust
pub trait AttestationProvider: Send + Sync {
    fn sign_settlement(&self, data: &SettlementData) -> anyhow::Result<Vec<u8>>;
    fn signing_address(&self) -> Option<String>;
}
```

Follows the existing `ChainClient` trait pattern (mock for tests, real for production).

### MockAttestationProvider

Returns empty bytes. Used in Tier 1–3 and unit tests.

```rust
pub struct MockAttestationProvider;

impl AttestationProvider for MockAttestationProvider {
    fn sign_settlement(&self, _data: &SettlementData) -> anyhow::Result<Vec<u8>> {
        Ok(vec![])
    }
    fn signing_address(&self) -> Option<String> { None }
}
```

### EcdsaAttestationProvider

Signs settlement data with a secp256k1 key using alloy's `PrivateKeySigner`.

```rust
pub struct EcdsaAttestationProvider {
    signer: PrivateKeySigner,
}
```

Settlement hash computation:

```rust
fn settlement_hash(data: &SettlementData) -> [u8; 32] {
    keccak256_concat(&[
        data.buyer_old_nullifier,
        data.seller_old_nullifier,
        data.buyer_new_commitment,
        data.seller_new_commitment,
    ])
}
```

Signing uses `signer.sign_hash(&hash)` which applies EIP-191 prefixing internally,
producing a 65-byte (r, s, v) signature that `ecrecover` can verify.

### Key Source

- **Phase A:** `TEE_ATTESTATION_KEY` env var (hex private key)
- **Phase B:** Sealed key file at `/sealed/attestation.key`, unsealed via Gramine's
  SGX sealing API. Generated on first boot if not present.

---

## 11.3 Rust — Integration Points

### `tee/src/state.rs`

Add attestation provider to state:

```rust
pub struct TeeState {
    // ... existing fields ...
    pub attestation: Arc<dyn AttestationProvider>,
}
```

Update `TeeState::new()` and `new_shared()` to accept the provider.

### `tee/src/main.rs`

Construct provider based on env vars:

```rust
let attestation: Arc<dyn AttestationProvider> = match env::var("TEE_ATTESTATION_KEY") {
    Ok(key) => {
        let provider = EcdsaAttestationProvider::new(&key)?;
        tracing::info!("Attestation signing address: {}", provider.signing_address().unwrap());
        Arc::new(provider)
    }
    Err(_) => {
        tracing::warn!("TEE_ATTESTATION_KEY not set — using mock attestation");
        Arc::new(MockAttestationProvider)
    }
};
```

### `tee/src/chain.rs`

Update trait signature:

```rust
async fn settle_match(&self, data: &SettlementData, attestation: Vec<u8>) -> anyhow::Result<()>;
```

`AlloyChainClient` passes `Bytes::from(attestation)` instead of `Bytes::new()`.
`MockChainClient` stores the attestation bytes for test assertions.

### `tee/src/routes/orders.rs`

After `generate_settlement()`, before `chain.settle_match()`:

```rust
let attestation_bytes = state.attestation.sign_settlement(&settlement)
    .unwrap_or_else(|e| {
        tracing::error!("attestation signing failed: {e}");
        vec![]
    });

// ... later, outside write lock:
chain.settle_match(&settlement, attestation_bytes).await
```

Clone `Arc<dyn AttestationProvider>` before dropping the write lock to avoid
holding it during signing.

---

## 11.4 Solidity — Contract Changes

### New State Variables

```solidity
/// @notice Enclave signing key (public key address for attestation verification)
address public enclaveSigningKey;

/// @notice Whether attestation is required for settlement
bool public requireAttestation;
```

### New Admin Functions

```solidity
function setEnclaveSigningKey(address _key) external onlyOwner {
    enclaveSigningKey = _key;
}

function setRequireAttestation(bool _required) external onlyOwner {
    requireAttestation = _required;
}
```

### settleMatch Verification

Replace the TODO block (lines 160–163 of `DePLOB.sol`):

```solidity
// Verify attestation
if (attestation.length > 0) {
    bytes32 settlementHash = keccak256(abi.encodePacked(
        buyerOldNullifier,
        sellerOldNullifier,
        buyerNewCommitment,
        sellerNewCommitment
    ));
    bytes32 ethSignedHash = keccak256(abi.encodePacked(
        "\x19Ethereum Signed Message:\n32",
        settlementHash
    ));

    require(attestation.length == 65, "Invalid attestation length");
    bytes32 r;
    bytes32 s;
    uint8 v;
    assembly {
        r := calldataload(attestation.offset)
        s := calldataload(add(attestation.offset, 32))
        v := byte(0, calldataload(add(attestation.offset, 64)))
    }
    address recovered = ecrecover(ethSignedHash, v, r, s);
    require(recovered != address(0) && recovered == enclaveSigningKey, "Invalid attestation");
} else {
    require(!requireAttestation, "Attestation required");
}
```

### Interface Update

Add to `IDePLOB.sol`:

```solidity
function setEnclaveSigningKey(address _key) external;
function setRequireAttestation(bool _required) external;
```

---

## 11.5 Testing

### Solidity Tests

Using Foundry's `vm.sign()` cheatcode:

| Test | Description |
|------|-------------|
| `test_SettleMatchWithValidAttestation` | Sign settlement hash with known key, verify success |
| `test_SettleMatchWithInvalidAttestation` | Wrong signer key, expect `"Invalid attestation"` revert |
| `test_SettleMatchNoAttestationAllowed` | Empty attestation, `requireAttestation = false` — should pass |
| `test_SettleMatchNoAttestationRequired` | Empty attestation, `requireAttestation = true` — should revert |
| `test_SetEnclaveSigningKeyOnlyOwner` | Non-owner call reverts |
| `test_SetRequireAttestationOnlyOwner` | Non-owner call reverts |

The `vm.sign(privateKey, hash)` cheatcode returns `(v, r, s)` which can be packed into
65 bytes for the `attestation` parameter.

### Rust Tests

In `tee/src/attestation.rs`:

| Test | Description |
|------|-------------|
| `test_mock_returns_empty` | MockAttestationProvider returns empty bytes |
| `test_ecdsa_produces_65_bytes` | EcdsaAttestationProvider signature is 65 bytes |
| `test_ecdsa_recovers_correct_address` | Recovered address matches signer address |

---

## 11.6 Gramine SGX Setup (Phase B)

### Prerequisites

- SGX-capable machine (Azure DCsv3/DCdsv3, or bare-metal with Intel SGX2)
- Ubuntu 22.04
- Intel SGX DCAP driver
- Gramine (v1.6+)

### Manifest Template

`tee/gramine/deplob-tee.manifest.template`:

```toml
[libos]
entrypoint = "deplob-tee"

[loader]
entrypoint = "file:{{ gramine.libos }}"
argv = ["deplob-tee"]
env.LD_LIBRARY_PATH = "/lib:/lib/x86_64-linux-gnu"
# Pass through required env vars
env.ETH_RPC_URL = { passthrough = true }
env.DEPLOB_ADDRESS = { passthrough = true }
env.TEE_PRIVATE_KEY = { passthrough = true }

[fs]
mounts = [
    { path = "/lib", uri = "file:{{ gramine.runtimedir() }}" },
    { path = "/sealed", uri = "file:sealed/", type = "encrypted", key_name = "_sgx_mrenclave" },
]

[sgx]
enclave_size = "256M"
max_threads = 32
debug = false
edmm_enable = false
remote_attestation = "dcap"

trusted_files = [
    "file:deplob-tee",
    "file:{{ gramine.libos }}",
    "file:{{ gramine.runtimedir() }}/",
]
```

### Build Steps

`tee/gramine/Makefile`:

```makefile
SGX_SIGNER_KEY ?= enclave-key.pem

.PHONY: all clean

all: deplob-tee.manifest.sgx deplob-tee.sig

deplob-tee:
	cargo build --release --manifest-path ../Cargo.toml
	cp ../target/release/deplob-tee .

deplob-tee.manifest: deplob-tee.manifest.template deplob-tee
	gramine-manifest \
		-Dlog_level=error \
		$< $@

deplob-tee.manifest.sgx deplob-tee.sig: deplob-tee.manifest deplob-tee
	gramine-sgx-sign \
		--manifest $< \
		--key $(SGX_SIGNER_KEY) \
		--output $@

clean:
	rm -f deplob-tee deplob-tee.manifest deplob-tee.manifest.sgx deplob-tee.sig
```

### Dockerfile

`tee/gramine/Dockerfile`:

```dockerfile
FROM ubuntu:22.04

# Install Gramine and SGX dependencies
RUN apt-get update && apt-get install -y \
    curl gnupg2 software-properties-common \
    && curl -fsSLo /usr/share/keyrings/gramine-keyring.gpg \
       https://packages.gramineproject.io/gramine-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/gramine-keyring.gpg] \
       https://packages.gramineproject.io/ jammy main" \
       > /etc/apt/sources.list.d/gramine.list \
    && apt-get update && apt-get install -y \
       gramine libsgx-dcap-ql libsgx-dcap-default-qpl \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /app
COPY . .

# Build and sign enclave
RUN cd tee/gramine && make

# Sealed key directory (persisted via volume)
RUN mkdir -p /app/tee/gramine/sealed

WORKDIR /app/tee/gramine
ENTRYPOINT ["gramine-sgx", "deplob-tee"]
```

### Key Sealing Flow

1. On first boot inside SGX, `EcdsaAttestationProvider` checks for `/sealed/attestation.key`
2. If not found: generate new secp256k1 key pair, write to `/sealed/attestation.key`
   (Gramine encrypts this with MRENCLAVE-derived key automatically)
3. If found: read and unseal (Gramine decrypts transparently)
4. Log the derived public address so the operator can register it on-chain

### MRENCLAVE Verification

After `gramine-sgx-sign`, the MRENCLAVE hash is printed. The operator verifies this
matches the expected value (from a reproducible build) before calling
`setEnclaveSigningKey(address)` on the contract.

---

## 11.7 Deployment

### Phase A (any machine, no SGX)

1. Generate attestation key: `cast wallet new`
2. Set env vars:
   ```
   TEE_ATTESTATION_KEY=0x<private_key>
   TEE_PRIVATE_KEY=0x<tx_sender_key>
   ETH_RPC_URL=<rpc>
   DEPLOB_ADDRESS=<contract>
   ```
3. Deploy contract, then call:
   ```
   cast send $DEPLOB_ADDRESS "setEnclaveSigningKey(address)" <attestation_address>
   cast send $DEPLOB_ADDRESS "setRequireAttestation(bool)" true
   ```
4. Run TEE: `cargo run -p deplob-tee --release`

### Phase B (SGX machine)

1. Provision SGX VM (Azure DCsv3 recommended)
2. Clone repo, build enclave: `cd tee/gramine && make`
3. Run: `gramine-sgx deplob-tee`
4. On first boot, TEE logs its enclave signing address
5. Owner calls `setEnclaveSigningKey(address)` with the logged address
6. Owner calls `setRequireAttestation(true)` to enforce attestation

### Key Rotation

If the enclave code changes (new MRENCLAVE), the sealed key cannot be unsealed.
A new key is generated automatically on next boot. The operator must call
`setEnclaveSigningKey` again with the new address.

---

## 11.8 Security Considerations

### Two Signing Keys

The `TEE_PRIVATE_KEY` (transaction sender) and `TEE_ATTESTATION_KEY` (attestation signer)
are intentionally separate. In Phase B, the attestation key is enclave-sealed and cannot
be extracted, while the transaction-sending key must be accessible to the wallet software.
Even if the operator wallet is compromised, the attacker cannot produce valid attestation
signatures without access to the enclave.

### Defense in Depth

The `onlyTEE` modifier (msg.sender check) remains as a first layer. The attestation
signature is a second layer. Both must pass for settlement to succeed.

### Future: On-Chain DCAP Verification

Phase B trusts the operator to verify MRENCLAVE out-of-band before registering the
signing key. A fully trustless approach would verify the SGX DCAP quote on-chain using
a library like Automata's DCAP verifier. This replaces `setEnclaveSigningKey` with:

```solidity
function registerEnclave(bytes calldata dcapQuote) external {
    // Verify DCAP quote on-chain
    // Extract enclave signing key from quote's report data
    // Store as enclaveSigningKey
}
```

This is documented as future work beyond the FYP scope.
