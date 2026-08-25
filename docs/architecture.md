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

    %% 5. Seller confirms swap (reveals decryption key; USDC stays escrowed)
    Seller->>AtomicSwap: confirm_swap(swap_id, decryption_key, proof_path)
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

    %% 7. USDC is only released once the dispute window and escrow hold both
    %% clear — see "Escrow Hold Period" below. In the dApp, the seller does
    %% this from the swap page's "Release Payment" button; no CLI call
    %% is required.
    Note over Seller,Buyer: See Escrow Hold Period for the release_to_seller step
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

### Pending-Swap Guard on Listing Mutations

`update_listing`, `transfer_listing_ownership`, `create_version`, and
`deregister_listing` all reject with `PendingSwapExists` while a swap is
`Pending` for the listing, so a seller can't edit, reassign, version, or
delete a listing out from under a buyer who is mid-swap.

**Enforced unconditionally, not caller-configurable.** The `atomic_swap`
contract consulted for this check is a single address stored in
`ip_registry`'s `Config`, set at `initialize` and changeable only by the
admin via `set_atomic_swap`. None of the four functions above accept an
`atomic_swap` address as a call parameter — the listing owner (who is
exactly the party this check restrains) has no way to point it at a
different contract, and `deregister_listing` has no way to skip the check
either. This closes off the alternative a caller-supplied address would
otherwise leave open: deploying a look-alike contract whose
`has_pending_swap` always returns `false`.

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
funds are currently time-locked; the frontend's `HoldPeriodDisplay` derives the
same result client-side from `get_swap`'s `hold_until`/`buyer_confirmed` fields
and the on-chain ledger timestamp, so its countdown can't drift on a skewed
local clock.

### In-app release flow

The swap page wires both payout paths directly, so a seller never needs the
CLI to actually collect payment:

- **Seller — "Release Payment"**: calls `release_to_seller(swap_id)` once the
  dispute window and escrow hold have both cleared. Disabled while the hold is
  still active.
- **Buyer — "Confirm Receipt & Release Early"**: calls `confirm_receipt(swap_id)`
  to waive the remaining hold once they're satisfied with the delivered IP.

The "Payment Received" notification only fires once a swap actually reaches
`ResolvedSeller` (funds moved), not when it merely reaches `Completed` (the key
was revealed but funds are still escrowed).

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

## Fee & Royalty Bounds

At settlement (`release_to_seller` and the seller-favoring branch of dispute
resolution), the seller receives `usdc_amount − protocol_fee − royalty`. Both
`fee_bps` (`atomic_swap` config) and `royalty_bps` (a listing's `ip_registry`
field) are independently bounded to `<= 10_000` (100%), but they live in two
separate contracts and are set at different times — a listing can be created
long before an admin later raises `fee_bps` via `update_config`.

- **Enforced invariant:** at settlement time, `fee_bps + effective_royalty_bps
  <= 10_000` always holds, so `seller_amount` can never go negative and the
  transfer can never panic.
- **Clamping, not rejection.** Because `update_config` cannot retroactively
  re-validate every existing listing (and shouldn't — it isn't the right layer
  to iterate storage), the combined-bps budget is enforced where it is
  actually spent: `effective_royalty_bps = min(listing.royalty_bps, 10_000 -
  config.fee_bps)`. Royalty is still computed on the gross sale price
  (`usdc_amount`), independent of the fee — clamping only caps how much of
  that budget the royalty portion can claim once the protocol fee has taken
  its share.
- **Auditability.** If a later fee change forces a listing's effective royalty
  below its nominal `royalty_bps`, a `RoyaltyClamped` event is published with
  both the nominal and effective values, so the degradation is visible
  off-chain even though settlement itself never fails.
- **Overflow-safe arithmetic.** Both the fee and royalty computations use
  `checked_mul`/`checked_div` against 10,000 (matching pattern), panicking
  with `ContractError::Overflow` rather than wrapping silently.
