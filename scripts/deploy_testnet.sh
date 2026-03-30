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
: "${ATOMIC_SWAP_EXPIRY_SECS:=86400}"
: "${IP_REGISTRY_TTL_THRESHOLD:=100000}"
: "${IP_REGISTRY_TTL_EXTEND_TO:=200000}"

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

echo "Initializing ip_registry contract..."
if ! stellar contract invoke \
  --id "$IP_REGISTRY" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- \
  initialize \
  --admin "$ATOMIC_SWAP_ADMIN" \
  --ttl_threshold "$IP_REGISTRY_TTL_THRESHOLD" \
  --ttl_extend_to "$IP_REGISTRY_TTL_EXTEND_TO"; then
  echo "Failed to initialize ip_registry contract: $IP_REGISTRY" >&2
  exit 1
fi

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
  --cancel_delay_secs "$ATOMIC_SWAP_CANCEL_DELAY_SECS" \
  --swap_expiry_secs "${ATOMIC_SWAP_EXPIRY_SECS:-86400}" \
  --zk_verifier "$ZK_VERIFIER" \
  --ip_registry "$IP_REGISTRY"; then
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
echo "  IP_REGISTRY : $IP_REGISTRY"
echo "  ATOMIC_SWAP : $ATOMIC_SWAP"
echo "  ZK_VERIFIER : $ZK_VERIFIER"
echo "=========================================="
echo "Updated .env with deployed contract IDs."
echo ""

echo "Running post-deployment smoke tests..."

echo "  [ip_registry] get_listing(listing_id=999) -> expect None (contract is callable)"
IP_REGISTRY_RESULT=$(stellar contract invoke \
  --id "$IP_REGISTRY" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- get_listing --listing_id 999 2>&1) || {
  echo "  ✗ FAILED: get_listing on ip_registry ($IP_REGISTRY)" >&2
  echo "    $IP_REGISTRY_RESULT" >&2
  exit 1
}
echo "  ✓ ip_registry responded: $IP_REGISTRY_RESULT"

echo "  [zk_verifier] get_merkle_root(listing_id=999) -> expect None (contract is callable)"
ZK_VERIFIER_RESULT=$(stellar contract invoke \
  --id "$ZK_VERIFIER" \
  --network "$STELLAR_NETWORK" \
  --source deployer \
  -- get_merkle_root --listing_id 999 2>&1) || {
  echo "  ✗ FAILED: get_merkle_root on zk_verifier ($ZK_VERIFIER)" >&2
  echo "    $ZK_VERIFIER_RESULT" >&2
  exit 1
}
echo "  ✓ zk_verifier responded: $ZK_VERIFIER_RESULT"

echo ""
echo "=========================================="
echo "✓ All smoke tests passed."
echo "  Contracts are live and callable on $STELLAR_NETWORK."
echo "=========================================="
