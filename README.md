# Atomic IP Marketplace

[![CI](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml/badge.svg)](https://github.com/unixfundz/Atomic-IP-Marketplace/actions/workflows/ci.yml)

Soroban smart contracts for atomic IP swaps using USDC, IP registry, and ZK verification.

## Overview
- **`atomic_swap`**: Atomic swaps with USDC payments, pause functionality, buyer/seller indexing.
- **`ip_registry`**: Register and query IP assets with TTL.
- **`zk_verifier`**: Merkle tree ZK proof verification with TTL.

See [contracts/](/contracts/) for sources and [docs/architecture.md](./docs/architecture.md) for sequence diagrams.

## Getting Started

### Prerequisites

- **Rust** (stable, with `wasm32-unknown-unknown` target)
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  rustup target add wasm32-unknown-unknown
  ```
- **Stellar CLI** — used to build, optimize, and deploy contracts
  ```bash
  cargo install --locked stellar-cli --features opt
  ```
- **Node.js** v18+ and npm — required for the frontend
  ```bash
  # via nvm (recommended)
  nvm install 18
  ```

### Clone and configure

```bash
git clone https://github.com/unixfundz/Atomic-IP-Marketplace.git
cd Atomic-IP-Marketplace
cp .env.example .env
```

### .env.example walkthrough

| Variable | Description |
|---|---|
| `STELLAR_NETWORK` | `testnet` or `mainnet` |
| `STELLAR_RPC_URL` | Soroban RPC endpoint (default: testnet) |
| `CONTRACT_ATOMIC_SWAP` | Deployed atomic_swap contract ID (filled after deploy) |
| `CONTRACT_IP_REGISTRY` | Deployed ip_registry contract ID |
| `CONTRACT_ZK_VERIFIER` | Deployed zk_verifier contract ID |
| `VITE_*` | Frontend copies of the above for Vite |
| `ATOMIC_SWAP_ADMIN` | Admin address for contract initialization |
| `ATOMIC_SWAP_FEE_RECIPIENT` | Address that receives protocol fees |
| `ATOMIC_SWAP_FEE_BPS` | Fee in basis points (e.g. `250` = 2.5%) |
| `ATOMIC_SWAP_CANCEL_DELAY_SECS` | Seconds before a buyer can cancel (e.g. `3600`) |

### Build contracts

```bash
# Build and optimize all contracts
./scripts/build.sh

# Build a single contract
./scripts/build.sh atomic_swap
# Available: ip_registry, atomic_swap, zk_verifier
```

### Run tests

```bash
./scripts/test.sh
```

This runs `cargo test --locked --workspace` across all three contracts.

### Local testnet setup

1. Start a local Stellar network (requires Docker):
   ```bash
   stellar network start local
   ```
2. Add a funded account:
   ```bash
   stellar keys generate deployer --network local
   stellar keys fund deployer --network local
   ```
3. Set `STELLAR_NETWORK=local` and `STELLAR_RPC_URL=http://localhost:8000/soroban/rpc` in `.env`.
4. Deploy:
   ```bash
   ./scripts/deploy_testnet.sh
   ```

### Frontend

```bash
cd frontend
cp .env.example .env   # fill in VITE_CONTRACT_* after deploy
npm install
npm run dev
```

---

## Build & Test

Build all contracts:
```bash
./scripts/build.sh
```

Run tests:
```bash
./scripts/test.sh
```

## Deploy (Testnet)
```bash
./scripts/deploy_testnet.sh
```

## Security

Please report vulnerabilities privately — do not open public issues for security bugs. See [SECURITY.md](./SECURITY.md) for the full responsible disclosure policy.

## License
This project is licensed under the Apache License 2.0. See the [LICENSE](./LICENSE) file for details.

---

*Workspace using pinned Soroban SDK v25.3.0 and soroban-poseidon v25.0.1*