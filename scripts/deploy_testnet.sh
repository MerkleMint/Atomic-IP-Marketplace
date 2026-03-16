#!/usr/bin/env bash
set -e
source .env

echo "Deploying to testnet..."

IP_REGISTRY=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/ip_registry.wasm \
  --network testnet \
  --source deployer)

ATOMIC_SWAP=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/atomic_swap.wasm \
  --network testnet \
  --source deployer)

ZK_VERIFIER=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/zk_verifier.wasm \
  --network testnet \
  --source deployer)

echo "CONTRACT_IP_REGISTRY=$IP_REGISTRY"
echo "CONTRACT_ATOMIC_SWAP=$ATOMIC_SWAP"
echo "CONTRACT_ZK_VERIFIER=$ZK_VERIFIER"
