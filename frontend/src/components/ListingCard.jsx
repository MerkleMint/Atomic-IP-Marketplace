import React from "react";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import "./ListingCard.css";

/**
 * ListingCard
 *
 * Displays a single IP listing owned by the connected seller.
 * Shows listing metadata and any pending swaps with a confirm-swap action.
 *
 * Props:
 *   listing       - { id, ipfs_hash, merkle_root, price_usdc, royalty_bps, activeSwaps[] }
 *   wallet        - connected wallet { address, signTransaction }
 *   onSwapUpdated - callback to refresh data after a swap action
 */
export function ListingCard({ listing, wallet, onSwapUpdated }) {
  const hasPendingSwaps = listing.activeSwaps.length > 0;

  return (
    <div className="listing-card" data-has-swaps={hasPendingSwaps}>
      {/* ── Listing metadata ── */}
      <div className="listing-card__header">
        <span className="listing-card__id">Listing #{listing.id}</span>
        {hasPendingSwaps && (
          <span className="listing-card__badge" aria-label={`${listing.activeSwaps.length} pending swap(s)`}>
            {listing.activeSwaps.length} pending
          </span>
        )}
      </div>

      <dl className="listing-card__meta">
        <div className="listing-card__meta-row">
          <dt>IPFS Hash</dt>
          <dd className="listing-card__hash" title={listing.ipfs_hash}>
            {listing.ipfs_hash ? `${listing.ipfs_hash.slice(0, 16)}…` : "—"}
          </dd>
        </div>
        <div className="listing-card__meta-row">
          <dt>Price</dt>
          <dd>{listing.price_usdc > 0 ? `${listing.price_usdc} USDC` : "Open"}</dd>
        </div>
        <div className="listing-card__meta-row">
          <dt>Royalty</dt>
          <dd>{listing.royalty_bps > 0 ? `${listing.royalty_bps / 100}%` : "None"}</dd>
        </div>
      </dl>

      {/* ── Pending swaps ── */}
      {hasPendingSwaps && (
        <div className="listing-card__swaps">
          <p className="listing-card__swaps-label">Pending Swaps</p>
          <ul className="listing-card__swaps-list">
            {listing.activeSwaps.map((swap) => (
              <li key={swap.id} className="listing-card__swap-item">
                <div className="listing-card__swap-meta">
                  <span className="listing-card__swap-id">Swap #{swap.id}</span>
                  <span className="listing-card__swap-amount">{swap.usdc_amount} USDC</span>
                  <span className="listing-card__swap-buyer" title={swap.buyer}>
                    Buyer: {swap.buyer.slice(0, 8)}…
                  </span>
                </div>
                <ConfirmSwapForm
                  swap={swap}
                  wallet={wallet}
                  onSuccess={onSwapUpdated}
                />
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
