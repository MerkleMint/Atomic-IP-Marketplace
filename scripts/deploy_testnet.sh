#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

if [[ -f .env ]]; then
  source .env
elif [[ -f .env.example ]]; then
  cp .env.example .env
  source .env
else
  echo "Missing .env and .env.example" >&2
  exit 1
fi

: "${STELLAR_NETWORK:=testnet}"
: "${ATOMIC_SWAP_ADMIN:?ATOMIC_SWAP_ADMIN must be set in .env}"
: "${ATOMIC_SWAP_FEE_BPS:=0}"
: "${ATOMIC_SWAP_FEE_RECIPIENT:?ATOMIC_SWAP_FEE_RECIPIENT must be set in .env}"
: "${ATOMIC_SWAP_CANCEL_DELAY_SECS:=3600}"

echo "Deploying to testnet..."

deploy_contract() {
  local wasm_path="$1"
  local deployed_id
  if ! deployed_id=$(stellar contract deploy \
    --wasm "$wasm_path" \
    --network "$STELLAR_NETWORK" \
    --source deployer); then
    echo "Failed to deploy contract wasm: $wasm_path" >&2
    exit 1
  fi
  printf '%s' "$deployed_id"
}

IP_REGISTRY=$(deploy_contract target/wasm32-unknown-unknown/release/ip_registry.wasm)
ATOMIC_SWAP=$(deploy_contract target/wasm32-unknown-unknown/release/atomic_swap.wasm)
ZK_VERIFIER=$(deploy_contract target/wasm32-unknown-unknown/release/zk_verifier.wasm)

echo "Initializing atomic swap contract..."
if ! stellar contract invoke \
  --id "$ATOMIC_SWAP" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- \
  initialize \
  --admin "$ATOMIC_SWAP_ADMIN" \
  --fee_bps "$ATOMIC_SWAP_FEE_BPS" \
  --fee_recipient "$ATOMIC_SWAP_FEE_RECIPIENT" \
  --cancel_delay_secs "$ATOMIC_SWAP_CANCEL_DELAY_SECS"; then
  echo "Failed to initialize atomic swap contract: $ATOMIC_SWAP" >&2
  exit 1
fi

set_env_var() {
  local key="$1"
  local value="$2"
  if grep -q "^${key}=" .env; then
    sed -i.bak "s|^${key}=.*|${key}=${value}|" .env
  else
    printf '\n%s=%s\n' "$key" "$value" >> .env
  fi
}

set_env_var CONTRACT_IP_REGISTRY "$IP_REGISTRY"
set_env_var CONTRACT_ATOMIC_SWAP "$ATOMIC_SWAP"
set_env_var CONTRACT_ZK_VERIFIER "$ZK_VERIFIER"
rm -f .env.bak

echo ""
echo "=========================================="
echo "Deployment complete!"
echo "=========================================="
echo "Contract addresses:"
echo "  IP_REGISTRY:   $IP_REGISTRY"
echo "  ATOMIC_SWAP:   $ATOMIC_SWAP"
echo "  ZK_VERIFIER:   $ZK_VERIFIER"
echo "=========================================="
echo "Updated .env with deployed contract IDs."
echo ""

echo ""
echo "Running post-deployment smoke tests..."

echo "  [ip_registry] get_listing(999) -> expect None"
if ! stellar contract invoke \
  --id "$IP_REGISTRY" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- get_listing --listing_id 999; then
  echo "Failed to invoke get_listing on ip_registry: $IP_REGISTRY" >&2
  exit 1
fi

echo "  [zk_verifier] get_merkle_root(999) -> expect None"
if ! stellar contract invoke \
  --id "$ZK_VERIFIER" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- get_merkle_root --listing_id 999; then
  echo "Failed to invoke get_merkle_root on zk_verifier: $ZK_VERIFIER" >&2
  exit 1
fi

echo ""
echo "✓ Smoke tests passed. All three contracts are live and callable."
