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
    AtomicSwap->>USDC: transfer(contract → fee_recipient, fee)
    AtomicSwap->>USDC: transfer(contract → seller, amount - fee)
    USDC-->>AtomicSwap: ok
    AtomicSwap-->>Seller: ok (status → Completed)

    %% 6. Buyer retrieves decryption key
    Buyer->>AtomicSwap: get_decryption_key(swap_id)
    AtomicSwap-->>Buyer: decryption_key
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
