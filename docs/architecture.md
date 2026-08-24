# Architecture: Sequence Diagrams

## Swap Lifecycle & ZK Proof Flow

The full happy-path flow spans three contracts: `ip_registry`, `zk_verifier`, and `atomic_swap`.

```mermaid
sequenceDiagram
    actor Seller
    actor Buyer
    participant IPRegistry as ip_registry
    participant ZKVerifier as zk_verifier
    participant AtomicSwap as atomic_swap
    participant USDC as USDC Token

    %% 1. Seller registers IP asset
    Seller->>IPRegistry: register_ip(owner, ipfs_hash, merkle_root)
    IPRegistry-->>Seller: listing_id

    %% 2. Seller commits Merkle root for ZK proofs
    Seller->>ZKVerifier: set_merkle_root(owner, listing_id, root)
    ZKVerifier-->>Seller: ok

    %% 3. Buyer verifies a partial proof before committing funds
    Buyer->>ZKVerifier: verify_partial_proof(listing_id, leaf, path)
    ZKVerifier-->>Buyer: true / false

    %% 4. Buyer initiates swap (locks USDC)
    Buyer->>AtomicSwap: initiate_swap(listing_id, buyer, seller, usdc_token, amount)
    AtomicSwap->>IPRegistry: get_listing(listing_id)
    IPRegistry-->>AtomicSwap: Listing { owner, ... }
    Note over AtomicSwap: asserts listing.owner == seller
    AtomicSwap->>USDC: transfer(buyer → contract, amount)
    USDC-->>AtomicSwap: ok
    AtomicSwap-->>Buyer: swap_id

    %% 5. Seller confirms swap (reveals decryption key, receives USDC)
    Seller->>AtomicSwap: confirm_swap(swap_id, decryption_key)
    Note over AtomicSwap: asserts swap.status == Pending
    AtomicSwap->>IPRegistry: get_listing(listing_id)
    IPRegistry-->>AtomicSwap: Listing { merkle_root, ... }
    AtomicSwap->>ZKVerifier: get_merkle_root(listing_id)
    ZKVerifier-->>AtomicSwap: merkle_root
    Note over AtomicSwap: reverts (MerkleRootMismatch) unless the two roots match
    AtomicSwap->>USDC: transfer(contract → fee_recipient, fee)
    AtomicSwap->>USDC: transfer(contract → seller, amount - fee)
    USDC-->>AtomicSwap: ok
    AtomicSwap-->>Seller: ok (status → Completed)

    %% 6. Buyer retrieves decryption key
    Buyer->>AtomicSwap: get_decryption_key(swap_id)
    AtomicSwap-->>Buyer: decryption_key
```

### Merkle Root Binding

`ip_registry` and `zk_verifier` each hold their own independently-writable copy
of a listing's Merkle root (`Listing.merkle_root` and
`DataKey::MerkleRoot(listing_id)`, respectively). Nothing prevented these from
drifting apart: a seller could advertise one root via `ip_registry` — the value
a buyer inspects with `get_listing` before committing funds — while a different
root is the one actually enforced by `zk_verifier.verify_partial_proof`.

**Source of truth: `ip_registry`.** It's what a buyer reads and reasons about
pre-swap, so it's the value being trusted. `zk_verifier`'s copy must match it,
not the other way around.

**Enforcement point: `atomic_swap::confirm_swap`.** The check is enforced at
confirm time — the single choke point where funds actually move — rather than
at `register_ip`/`update_listing`/`create_version`. Before verifying the ZK
proof, `confirm_swap` reads `ip_registry.get_listing(listing_id).merkle_root`
and `zk_verifier.get_merkle_root(listing_id)` and reverts with
`MerkleRootMismatch` if they differ (or if `zk_verifier` has no root
registered), before any proof verification runs.

Enforcing at registration time instead was rejected: it would force
`ip_registry` to take a hard dependency on `zk_verifier` on every mutation
path, and would break a seller who legitimately sets the root in `zk_verifier`
*after* creating the listing. Checking only at the swap choke point avoids a
redundant dual-write burden on every listing edit — only the swap path needs
the guard.

---

## Cancel / Refund Flow

If the seller never calls `confirm_swap` before the timeout, the buyer can reclaim their USDC.

```mermaid
sequenceDiagram
    actor Seller
    actor Buyer
    participant IPRegistry as ip_registry
    participant AtomicSwap as atomic_swap
    participant USDC as USDC Token

    %% Setup: swap is already initiated
    Buyer->>AtomicSwap: initiate_swap(listing_id, buyer, seller, ...)
    AtomicSwap->>IPRegistry: get_listing(listing_id)
    IPRegistry-->>AtomicSwap: Listing { owner, ... }
    AtomicSwap->>USDC: transfer(buyer → contract, amount)
    AtomicSwap-->>Buyer: swap_id

    %% Seller goes silent — timeout elapses
    Note over Seller,AtomicSwap: cancel_delay_secs elapses without confirm_swap

    %% Buyer cancels and reclaims USDC
    Buyer->>AtomicSwap: cancel_swap(swap_id)
    Note over AtomicSwap: asserts swap.status == Pending
    Note over AtomicSwap: asserts ledger.timestamp >= swap.expires_at
    AtomicSwap->>USDC: transfer(contract → buyer, amount)
    USDC-->>AtomicSwap: ok
    AtomicSwap-->>Buyer: ok (status → Cancelled)
```

## Escrow Hold Period

After a swap is completed (the seller submits the decryption key), the seller's
payout can be held for an additional, **seller-configurable** window layered on
top of the dispute window. This gives an honest buyer time to verify the
delivered IP before funds move, and lets a reputable seller advertise a buyer
protection window.

### Economics & rationale

- **Default: 24 hours** (`DEFAULT_HOLD_PERIOD_SECS = 86_400`). Applied when a
  seller enables holds without choosing a custom value.
- **Per-seller configuration.** A seller calls `set_seller_hold_period(seller,
  secs)`. A value of `0` opts out entirely. Sellers competing for buyers can
  offer a longer hold as a trust signal; sellers who want faster settlement can
  shorten or disable it.
- **Global default.** The admin may enable holds protocol-wide and set the
  default via `set_escrow_hold_config(enabled, default_secs)`. A per-seller
  override always wins over the global default.
- **Buyer confirmation override.** Once satisfied, the buyer calls
  `confirm_receipt(swap_id)` to waive the remaining hold, so the seller is paid
  immediately. This keeps a cooperative buyer from needlessly delaying an
  honest seller while still defaulting to buyer protection.

### Security properties

- **No manipulation after the fact.** `hold_until` is snapshotted at
  `confirm_swap` time from the seller's then-current configuration. Changing the
  per-seller or global setting afterwards cannot shorten or extend the hold on a
  swap that is already in progress.
- **Bounded duration.** Both setters reject values above
  `MAX_HOLD_PERIOD_SECS = 2_592_000` (30 days), preventing a hold from locking
  buyer funds indefinitely.
- **Authorization.** Only the buyer can call `confirm_receipt`; only the seller
  can set their own hold period; only the admin can change the global default.
- **Independent gate.** `release_to_seller` enforces the hold (error
  `HoldPeriodActive`) in addition to the dispute window — both must clear before
  funds are released, unless the buyer has confirmed receipt.

### Audit trail

`SellerHoldPeriodUpdated`, `EscrowHoldConfigUpdated`, and `BuyerConfirmedReceipt`
events provide an on-chain record of configuration changes and early-release
authorizations. The `hold_period_active(swap_id)` view returns whether a swap's
funds are currently time-locked, which the UI surfaces via `HoldPeriodDisplay`.

```mermaid
sequenceDiagram
    actor Seller
    actor Buyer
    participant AtomicSwap as atomic_swap
    participant USDC as USDC Token

    Seller->>AtomicSwap: set_seller_hold_period(seller, 86400)
    Note over AtomicSwap: bounded by MAX_HOLD_PERIOD_SECS

    Seller->>AtomicSwap: confirm_swap(swap_id, key, proof)
    Note over AtomicSwap: status → Completed; hold_until = now + hold_secs

    alt Buyer confirms receipt early
        Buyer->>AtomicSwap: confirm_receipt(swap_id)
        Note over AtomicSwap: buyer_confirmed = true (hold waived)
    else Hold period elapses
        Note over AtomicSwap: ledger.timestamp >= hold_until
    end

    Seller->>AtomicSwap: release_to_seller(swap_id)
    Note over AtomicSwap: requires dispute window cleared AND hold not active
    AtomicSwap->>USDC: transfer(contract → seller, amount − fees)
    AtomicSwap-->>Seller: ok (status → ResolvedSeller)
```
