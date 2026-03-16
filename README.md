# The Sovereign Patent — Atomic IP Marketplace

A trustless intellectual property exchange built on Stellar Soroban smart contracts, using Zero-Knowledge proofs and atomic swaps to eliminate IP theft during the negotiation phase.

Engineering designs are stolen every day during the "NDA phase." The Sovereign Patent solves this with a **Trustless Reveal** — you never expose your secret until payment is guaranteed, and payment is never released until the secret is delivered.

## 🎯 What is The Sovereign Patent?

Traditional IP sales require trust: you share the design, hope the buyer pays, and pray they don't steal it. This project replaces trust with cryptographic guarantees.

A seller uploads an encrypted CAD file (or any IP asset) to IPFS and stores its hash on Stellar. A buyer wants the design. Both parties enter a Soroban Atomic Swap — the buyer sends USDC, the seller sends the decryption key. The transaction either completes atomically or reverts entirely. There is no in-between.

The rare edge: a **Poseidon-based Merkle Tree** lets the buyer verify specific parts of the design (e.g., a gear ratio, a circuit spec, a formula) before purchasing the full file — without the seller ever revealing the whole secret.

This makes The Sovereign Patent:

✅ Trustless (no escrow agent, no NDA required)
✅ Atomic (payment and key delivery happen in one transaction or not at all)
✅ Verifiable (buyers confirm partial proofs before committing)
✅ Private (the full IP is never exposed until payment clears)
✅ Censorship-resistant (files live on IPFS, hashes anchored on Stellar)

## 🚀 Features

- **Atomic IP Swap**: Buyer sends USDC; seller sends decryption key — settled in a single Soroban transaction
- **Encrypted IPFS Storage**: CAD files, schematics, and designs are AES-encrypted before upload; only the hash is stored on-chain
- **ZK Partial Proofs**: Poseidon-based Merkle Trees allow buyers to verify specific design attributes without seeing the full file
- **Trustless Reveal**: The decryption key is only released when payment is confirmed — eliminating NDA-phase theft
- **USDC Native**: Built on Stellar's USDC for stable, borderless payments
- **On-chain IP Registry**: Design hashes and ownership records stored immutably on Stellar

## 🛠️ Quick Start

### Prerequisites

- Rust (1.70+)
- Soroban CLI
- Stellar CLI
- IPFS node or Pinata API key

### Build

```bash
./scripts/build.sh
```

### Test

```bash
./scripts/test.sh
```

### Setup Environment

Copy the example environment file:

```bash
cp .env.example .env
```

Configure your environment variables in `.env`:

```bash
# Network configuration
STELLAR_NETWORK=testnet
STELLAR_RPC_URL=https://soroban-testnet.stellar.org

# Contract addresses (after deployment)
CONTRACT_ATOMIC_SWAP=<your-contract-id>
CONTRACT_IP_REGISTRY=<your-contract-id>
CONTRACT_ZK_VERIFIER=<your-contract-id>

# IPFS configuration
IPFS_GATEWAY=https://gateway.pinata.cloud
PINATA_API_KEY=<your-pinata-api-key>

# Frontend configuration
VITE_STELLAR_NETWORK=testnet
VITE_STELLAR_RPC_URL=https://soroban-testnet.stellar.org
```

Network configurations are defined in `environments.toml`:

- `testnet` — Stellar testnet
- `mainnet` — Stellar mainnet
- `futurenet` — Stellar futurenet
- `standalone` — Local development

### Deploy to Testnet

```bash
# Configure your testnet identity first
stellar keys generate deployer --network testnet

# Deploy
./scripts/deploy_testnet.sh
```

### Run Demo

Follow the step-by-step guide in `demo/demo-script.md`

## 📖 Documentation

- [Architecture Overview](docs/architecture.md)
- [ZK Proof Design](docs/zk-proofs.md)
- [Threat Model & Security](docs/threat-model.md)
- [Roadmap](docs/roadmap.md)

## 🎓 Smart Contract API

### IP Registry

```rust
register_ip(owner, ipfs_hash, merkle_root) -> u64
get_listing(listing_id) -> Listing
list_by_owner(owner) -> Vec<Listing>
```

### Atomic Swap

```rust
initiate_swap(listing_id, buyer, usdc_amount) -> SwapId
confirm_swap(swap_id, decryption_key)
cancel_swap(swap_id)
get_swap_status(swap_id) -> SwapStatus
```

### ZK Verifier

```rust
verify_partial_proof(listing_id, proof, public_inputs) -> bool
get_merkle_root(listing_id) -> Hash
submit_proof(listing_id, leaf, path) -> bool
```

## 🧪 Testing

Comprehensive test suite covering:

✅ IP registration and hash anchoring
✅ Atomic swap initiation and settlement
✅ Decryption key release on payment confirmation
✅ Swap cancellation and refund logic
✅ ZK partial proof generation and verification
✅ Merkle tree construction and path validation
✅ Error handling and edge cases

Run tests:

```bash
cargo test
```

## 🔐 How the Trustless Reveal Works

1. **Seller** encrypts their IP file (CAD, schematic, code, etc.) and uploads it to IPFS
2. **Seller** builds a Poseidon Merkle Tree over the design's attributes and registers the root on-chain
3. **Buyer** requests a partial proof — e.g., "prove the gear ratio is 3:1" — without seeing the full file
4. **Seller** generates a ZK proof for that leaf; the on-chain verifier confirms it against the Merkle root
5. **Buyer** is satisfied and initiates an atomic swap: USDC locked in the contract
6. **Seller** submits the decryption key; the contract releases USDC atomically
7. **Buyer** decrypts the IPFS file — the full design is now theirs

If the seller never submits the key, the buyer's USDC is refunded. If the buyer never sends USDC, the key is never exposed. Neither party can cheat.

## 🌍 Why This Matters

IP theft during negotiation costs inventors billions annually. NDAs are legally enforceable but practically weak — especially across borders. Cryptographic guarantees don't require lawyers or jurisdiction.

**Target Users:**

- Independent engineers and inventors
- Hardware startups sharing designs with manufacturers
- Researchers licensing algorithms or datasets
- Creators selling proprietary formulas, blueprints, or schematics
- Any IP owner who has ever been burned by an NDA

## 🗺️ Roadmap

- **v1.0 (Current)**: Atomic swap for encrypted IPFS assets, USDC settlement
- **v1.1**: Poseidon Merkle Tree partial proofs, ZK verifier contract
- **v2.0**: Multi-asset support (XLM, EURC), time-locked escrow
- **v3.0**: Frontend UI with wallet integration and proof explorer
- **v4.0**: Mobile app, IP marketplace discovery layer

See [docs/roadmap.md](docs/roadmap.md) for details.

## 🤝 Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See our [Code of Conduct](CODE_OF_CONDUCT.md) and [Contributing Guidelines](CONTRIBUTING.md).

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Stellar Development Foundation](https://stellar.org) for Soroban
- The ZK research community for Poseidon hash and Merkle proof primitives
- Every engineer who lost IP to a bad-faith NDA — this is for you
