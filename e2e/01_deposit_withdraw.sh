#!/usr/bin/env bash
# Scenario 1: Deposit → Withdraw from different address (privacy proof)
#
# Proves: a user can deposit tokens, then withdraw to a completely fresh wallet
# with no on-chain link between the deposit and withdrawal addresses.
#
# Usage: TIER=local bash e2e/01_deposit_withdraw.sh
#        TIER=sepolia bash e2e/01_deposit_withdraw.sh

set -euo pipefail

TIER="${TIER:-local}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
set -a
source "$SCRIPT_DIR/.env.$TIER"
set +a

echo "============================================"
echo "  Scenario 1: Deposit → Withdraw"
echo "  Tier: $TIER"
echo "============================================"

# Step 1: Deposit TokenA
echo ""
echo "--- Step 1: Deposit 1000 TokenA ---"
cargo run -p deplob-cli --release -- deposit \
    --token "$TOKEN_A" \
    --amount 1000 \
    --note-file /tmp/e2e_note1.json \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY"

echo ""
echo "--- Deposit note ---"
cat /tmp/e2e_note1.json

# Wait for deposit to be indexed by RPC
echo ""
echo "--- Waiting 10s for deposit to be indexed... ---"
sleep 10

# Step 2: Withdraw to fresh wallet
echo ""
echo "--- Step 2: Withdraw to fresh wallet $FRESH_WALLET_2 ---"
cargo run -p deplob-cli --release -- withdraw \
    --note /tmp/e2e_note1.json \
    --recipient "$FRESH_WALLET_2" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER2_PRIVATE_KEY" \
    $PROVE_FLAG

echo ""
echo "============================================"
echo "  Scenario 1 COMPLETE"
echo "  Deposited from User1, withdrew to $FRESH_WALLET_2"
echo "  No on-chain link between addresses!"
echo "============================================"
