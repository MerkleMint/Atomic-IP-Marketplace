# Atomic IP Marketplace

[![CI](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml/badge.svg)](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml)

Soroban smart contracts for atomic IP swaps using USDC, IP registry, and ZK verification.

## Overview
- **`atomic_swap`**: Atomic swaps with USDC payments, pause functionality, buyer/seller indexing.
- **`ip_registry`**: Register and query IP assets with TTL.
- **`zk_verifier`**: Merkle tree ZK proof verification with TTL.

See [contracts/](/contracts/) for sources.

## Build & Test
```bash
./scripts/build.sh
./scripts/test.sh
```

## Deploy (Testnet)
```bash
./scripts/deploy_testnet.sh
```

## Security
[SECURITY.md](./SECURITY.md)

## License
TBD (add LICENSE file if needed).

---

*Workspace using Soroban SDK v22.0.0*
