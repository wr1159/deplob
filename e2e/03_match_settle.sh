#!/usr/bin/env bash
# Scenario 3: Two users deposit → Orders match → Withdraw swapped tokens
#
# Proves: the full trade flow — two users deposit different tokens, their orders
# match in the TEE, settlement happens on-chain, and both users withdraw their
# new tokens from fresh wallets.
#
# Usage: TIER=local bash e2e/03_match_settle.sh
#        TIER=sepolia bash e2e/03_match_settle.sh

set -euo pipefail

TIER="${TIER:-local}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/.env.$TIER"

echo "============================================"
echo "  Scenario 3: Two Users → Match → Settle → Withdraw"
echo "  Tier: $TIER"
echo "============================================"

# Step 1: User A deposits TokenA (quote token, amount = price * quantity = 2 * 100 = 200)
echo ""
echo "--- Step 1: User A deposits 200 TokenA ---"
cargo run -p deplob-cli --release -- deposit \
    --token "$TOKEN_A" \
    --amount 200 \
    --note-file /tmp/e2e_note_a.json \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY"

# Step 2: User B deposits TokenB (base token, amount = quantity = 100)
echo ""
echo "--- Step 2: User B deposits 100 TokenB ---"
cargo run -p deplob-cli --release -- deposit \
    --token "$TOKEN_B" \
    --amount 100 \
    --note-file /tmp/e2e_note_b.json \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER2_PRIVATE_KEY"

# Step 3: User A creates a buy order (buy 100 TokenB at price 2, paying with TokenA)
echo ""
echo "--- Step 3: User A creates buy order ---"
cargo run -p deplob-cli --release -- order \
    --note /tmp/e2e_note_a.json \
    --side buy \
    --price 2 \
    --quantity 100 \
    --token-in "$TOKEN_A" \
    --token-out "$TOKEN_B" \
    --tee-url "$TEE_URL" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY"

# Step 4: User B creates a sell order (sell 100 TokenB at price 2 → triggers match)
echo ""
echo "--- Step 4: User B creates sell order (triggers match) ---"
cargo run -p deplob-cli --release -- order \
    --note /tmp/e2e_note_b.json \
    --side sell \
    --price 2 \
    --quantity 100 \
    --token-in "$TOKEN_B" \
    --token-out "$TOKEN_A" \
    --tee-url "$TEE_URL" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER2_PRIVATE_KEY"

echo ""
echo "--- Waiting for settlement to be processed... ---"
sleep 3

# Step 5: Retrieve new deposit notes
echo ""
echo "--- Step 5: User A retrieves new deposit note ---"
cargo run -p deplob-cli --release -- settlement \
    --note /tmp/e2e_note_a.json \
    --tee-url "$TEE_URL" \
    --save /tmp/e2e_new_a.json

echo ""
echo "--- Step 5b: User B retrieves new deposit note ---"
cargo run -p deplob-cli --release -- settlement \
    --note /tmp/e2e_note_b.json \
    --tee-url "$TEE_URL" \
    --save /tmp/e2e_new_b.json

# Step 6: Withdraw swapped tokens from fresh wallets
echo ""
echo "--- Step 6: User A withdraws TokenB to fresh wallet ---"
cargo run -p deplob-cli --release -- withdraw \
    --note /tmp/e2e_new_a.json \
    --recipient "$FRESH_WALLET_1" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER1_PRIVATE_KEY" \
    $PROVE_FLAG

echo ""
echo "--- Step 6b: User B withdraws TokenA to fresh wallet ---"
cargo run -p deplob-cli --release -- withdraw \
    --note /tmp/e2e_new_b.json \
    --recipient "$FRESH_WALLET_2" \
    --rpc-url "$ETH_RPC_URL" \
    --contract "$DEPLOB_ADDRESS" \
    --private-key "$USER2_PRIVATE_KEY" \
    $PROVE_FLAG

# Cleanup
rm -f order_*.json

echo ""
echo "============================================"
echo "  Scenario 3 COMPLETE"
echo "  User A: deposited TokenA → received TokenB at $FRESH_WALLET_1"
echo "  User B: deposited TokenB → received TokenA at $FRESH_WALLET_2"
echo "  Full privacy-preserving token swap!"
echo "============================================"
