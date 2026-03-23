#!/usr/bin/env bash
# Setup Phase A attestation: generate keypair, update .env.sepolia, register on-chain
#
# Usage: DEPLOYER_PRIVATE_KEY=0x... bash e2e/setup_attestation.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ENV_FILE="$SCRIPT_DIR/.env.sepolia"

set -a
source "$ENV_FILE"
set +a

if [ -z "${DEPLOYER_PRIVATE_KEY:-}" ]; then
    echo "ERROR: DEPLOYER_PRIVATE_KEY must be set (contract owner key)"
    exit 1
fi

echo "============================================"
echo "  Phase A: Setup ECDSA Attestation"
echo "============================================"

# Step 1: Generate attestation keypair
echo ""
echo "--- Step 1: Generate attestation keypair ---"
WALLET_OUTPUT=$(cast wallet new 2>&1)
ATTESTATION_PRIVATE_KEY=$(echo "$WALLET_OUTPUT" | grep "Private key:" | awk '{print $3}')
ATTESTATION_ADDRESS=$(echo "$WALLET_OUTPUT" | grep "Address:" | awk '{print $2}')

echo "Attestation private key: $ATTESTATION_PRIVATE_KEY"
echo "Attestation address:     $ATTESTATION_ADDRESS"

# Step 2: Update .env.sepolia with the attestation key
echo ""
echo "--- Step 2: Update .env.sepolia ---"
sed -i '' "s|^TEE_ATTESTATION_KEY=.*|TEE_ATTESTATION_KEY=$ATTESTATION_PRIVATE_KEY|" "$ENV_FILE"
echo "Updated TEE_ATTESTATION_KEY in $ENV_FILE"

# Step 3: Register enclave signing key on-chain
echo ""
echo "--- Step 3: Register enclaveSigningKey on-chain ---"
cast send "$DEPLOB_ADDRESS" \
    "setEnclaveSigningKey(address)" "$ATTESTATION_ADDRESS" \
    --rpc-url "$ETH_RPC_URL" \
    --private-key "$DEPLOYER_PRIVATE_KEY"

echo "Registered enclaveSigningKey = $ATTESTATION_ADDRESS"

# Step 4: Enable attestation requirement
echo ""
echo "--- Step 4: Enable requireAttestation ---"
cast send "$DEPLOB_ADDRESS" \
    "setRequireAttestation(bool)" true \
    --rpc-url "$ETH_RPC_URL" \
    --private-key "$DEPLOYER_PRIVATE_KEY"

echo "Set requireAttestation = true"

# Step 5: Verify on-chain state
echo ""
echo "--- Step 5: Verify on-chain ---"
ONCHAIN_KEY=$(cast call "$DEPLOB_ADDRESS" "enclaveSigningKey()(address)" --rpc-url "$ETH_RPC_URL")
ONCHAIN_REQ=$(cast call "$DEPLOB_ADDRESS" "requireAttestation()(bool)" --rpc-url "$ETH_RPC_URL")
echo "On-chain enclaveSigningKey:  $ONCHAIN_KEY"
echo "On-chain requireAttestation: $ONCHAIN_REQ"

echo ""
echo "============================================"
echo "  Phase A Setup COMPLETE"
echo ""
echo "  Next steps:"
echo "  1. Start TEE: source e2e/.env.sepolia && cargo run -p deplob-tee --release"
echo "  2. Run scenario 3: TIER=sepolia bash e2e/03_match_settle.sh"
echo "============================================"
