#!/usr/bin/env bash
# Scenario 2: Deposit → Create order → Cancel → Withdraw
#
# Proves: the full order lifecycle works — user can deposit, place an order,
# cancel it (getting their deposit unlocked), and then withdraw.
#
# Usage: TIER=local bash e2e/02_order_cancel.sh
#        TIER=sepolia bash e2e/02_order_cancel.sh

set -euo pipefail

TIER="${TIER:-local}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/.env.$TIER"

echo "============================================"
echo "  Scenario 2: Deposit → Order → Cancel → Withdraw"
echo "  Tier: $TIER"
echo "============================================"

# Step 1: Deposit TokenA
echo ""
echo "--- Step 1: Deposit 1000 TokenA ---"
cargo run -p deplob-cli --release -- deposit \
    --token "$TOKEN_A" \
    --amount 1000 \
    --note-file /tmp/e2e_note2.json \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY"

# Step 2: Create order (sell 500 TokenA for TokenB at price 2)
echo ""
echo "--- Step 2: Create sell order ---"
cargo run -p deplob-cli --release -- order \
    --note /tmp/e2e_note2.json \
    --side sell \
    --price 2 \
    --quantity 500 \
    --token-in "$TOKEN_A" \
    --token-out "$TOKEN_B" \
    --tee-url "$TEE_URL" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY"

# Extract order_id from the saved order file
ORDER_FILE=$(ls -t order_0x*.json 2>/dev/null | head -1)
if [ -z "$ORDER_FILE" ]; then
    echo "ERROR: No order file found"
    exit 1
fi
ORDER_ID=$(python3 -c "import json; print(json.load(open('$ORDER_FILE'))['order_id'])")
echo "Order ID: $ORDER_ID"

# Step 3: Cancel the order
echo ""
echo "--- Step 3: Cancel order ---"
cargo run -p deplob-cli --release -- cancel \
    --order-id "$ORDER_ID" \
    --note /tmp/e2e_note2.json \
    --tee-url "$TEE_URL"

# Step 4: Withdraw (deposit is now unlocked after cancel)
echo ""
echo "--- Step 4: Withdraw to fresh wallet ---"
cargo run -p deplob-cli --release -- withdraw \
    --note /tmp/e2e_note2.json \
    --recipient "$FRESH_WALLET_1" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY" \
    $PROVE_FLAG

# Cleanup
rm -f "$ORDER_FILE"

echo ""
echo "============================================"
echo "  Scenario 2 COMPLETE"
echo "  Order created, cancelled, and deposit withdrawn"
echo "============================================"
