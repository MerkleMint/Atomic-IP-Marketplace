# Atomic IP Marketplace

[![CI](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml/badge.svg)](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml)

Soroban smart contracts for atomic IP swaps using USDC, IP registry, and ZK verification.

## Overview
- **`atomic_swap`**: Atomic swaps with USDC payments, pause functionality, buyer/seller indexing.
- **`ip_registry`**: Register and query IP assets with TTL.
- **`zk_verifier`**: Merkle tree ZK proof verification with TTL.

See [contracts/](/contracts/) for sources and [docs/architecture.md](./docs/architecture.md) for sequence diagrams.

## Build & Test

Build all contracts:
```bash
./scripts/build.sh
```

Build a specific contract:
```bash
./scripts/build.sh <contract_name>
```

Available contracts: `ip_registry`, `atomic_swap`, `zk_verifier`

Example:
```bash
./scripts/build.sh atomic_swap
```

Run tests:
```bash
./scripts/test.sh
```

## Deploy (Testnet)
```bash
./scripts/deploy_testnet.sh
```

## Frontend - Atomic IP Marketplace (Issue #34 + #89)

Full buyer UI:

### 1. Initiate Swap Flow
1. Deploy contracts (`./scripts/deploy_testnet.sh`, note IDs in `.env`)
2. Open `frontend/index.html`
3. Browse listings (demo data; real: query ip_registry.list_by_owner)
4. Click "Initiate Swap" → enter USDC amount
5. Approve USDC spend → Initiate (stubs; extend Freighter + full RPC XDR)
6. Note swap_id for key reveal

### 2. Key Reveal (post-swap)
Input atomic_swap ID + swap_id → fetch status/key

**Note:** Vanilla JS + Stellar SDK CDN. Stubs for RPC/wallet. Update CONTRACT_IDS in app.js with deployed IDs.

## Security
[SECURITY.md](./SECURITY.md)

## License
This project is licensed under the Apache License 2.0. See the [LICENSE](./LICENSE) file for details.

---

*Workspace using Soroban SDK v22.0.0*

