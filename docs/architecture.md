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
    AtomicSwap-->>Seller: ok (status → Completed; hold_until snapshotted)

    %% 6. Buyer retrieves decryption key
    Buyer->>AtomicSwap: get_decryption_key(swap_id)
    AtomicSwap-->>Buyer: decryption_key

    %% 7. USDC is only released once the dispute window and escrow hold both
    %% clear — see "Escrow Hold Period" below. In the dApp, the seller does
    %% this from the swap page's "Release Payment" button; no CLI call
    %% is required.
    Note over Seller,Buyer: See Escrow Hold Period for the release_to_seller step
```

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

## Dispute Appeal Flow

After arbiters commit/reveal votes on a raised dispute, `finalize_dispute`
tallies the vote weights and records an outcome — but escrow stays **held**,
not paid out, until the appeal window closes. This holdback is what makes an
appeal an actual remedy: since nothing was transferred yet, an appeal can
change the outcome with a single clean payout instead of clawing back funds
already sent to the losing party's counterpart.

- **`finalize_dispute`** (anyone, no auth): tallies revealed vote weights,
  sets `dispute.outcome` and `appeal_deadline_ledger`, and moves
  `swap.status` to `PendingAppealWindow`. No funds move.
- **No appeal filed:** once `appeal_deadline_ledger` passes, anyone may call
  `settle_dispute` to pay out per the recorded arbiter outcome.
- **Appeal filed:** only the buyer may call `appeal_dispute`, and only before
  `appeal_deadline_ledger`. This moves `swap.status` to `Appealed` and sets
  `appeal_resolution_deadline_ledger`. From here:
  - The **admin** may call `resolve_dispute` at any time to make the single,
    final payout — reversing the arbiter outcome if warranted.
  - If the admin never acts, anyone may call `settle_dispute` once
    `appeal_resolution_deadline_ledger` passes; it pays out per the
    **original** arbiter outcome, so an inactive admin can never
    permanently lock the escrow.

```mermaid
sequenceDiagram
    actor Buyer
    actor Admin
    participant AtomicSwap as atomic_swap
    participant USDC as USDC Token

    Note over AtomicSwap: arbiters commit/reveal votes (swap.status == Disputed)

    AtomicSwap->>AtomicSwap: finalize_dispute(swap_id)
    Note over AtomicSwap: outcome recorded; status → PendingAppealWindow (escrow held)

    alt No appeal before appeal_deadline_ledger
        AtomicSwap->>AtomicSwap: settle_dispute(swap_id)
        AtomicSwap->>USDC: transfer(contract → winner, amount − fees)
        AtomicSwap-->>AtomicSwap: ok (status → ResolvedBuyer/ResolvedSeller)
    else Buyer appeals before appeal_deadline_ledger
        Buyer->>AtomicSwap: appeal_dispute(swap_id, buyer)
        Note over AtomicSwap: status → Appealed; appeal_resolution_deadline_ledger set

        alt Admin resolves the appeal
            Admin->>AtomicSwap: resolve_dispute(swap_id, favor_buyer)
            AtomicSwap->>USDC: transfer(contract → winner, amount − fees)
            AtomicSwap-->>Admin: ok (status → ResolvedBuyer/ResolvedSeller)
        else Admin never acts — resolution timeout elapses
            AtomicSwap->>AtomicSwap: settle_dispute(swap_id)
            Note over AtomicSwap: pays per the ORIGINAL arbiter outcome
            AtomicSwap->>USDC: transfer(contract → winner, amount − fees)
            AtomicSwap-->>AtomicSwap: ok (status → ResolvedBuyer/ResolvedSeller)
        end
    end
```
