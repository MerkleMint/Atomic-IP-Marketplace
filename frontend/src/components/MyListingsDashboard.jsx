import React, { useState } from "react";
import { useWallet } from "../context/WalletContext";
import { useMyListings } from "../hooks/useMyListings";
import { ConfirmSwapForm } from "./ConfirmSwapForm";
import "./MyListingsDashboard.css";

/**
 * ListingCard
 *
 * Displays a single IP listing with its IPFS hash, listing ID,
 * and any active pending swap.
 *
 * Props:
 *   listing  - { id, owner, ipfs_hash, merkle_root, price_usdc, active_swap }
 *   wallet   - connected wallet { address, signTransaction }
 *   onSwapUpdated - callback to refresh data
 */
function ListingCard({ listing, wallet, onSwapUpdated }) {
  const hasActiveSwap = listing.active_swap && listing.active_swap.status === "Pending";
  const displayIpfsHash = listing.ipfs_hash.substring(0, 16) + "...";

  return (
    <div className="listing-card">
      <div className="listing-card__header">
        <span className="listing-card__id">Listing #{listing.id}</span>
        {hasActiveSwap && (
          <span className="listing-card__badge" data-status="pending">
            Pending Swap
          </span>
        )}
      </div>

      <div className="listing-card__info">
        <div className="listing-card__row">
          <span className="listing-card__label">IPFS Hash:</span>
          <span className="listing-card__value" title={listing.ipfs_hash}>
            {displayIpfsHash}
          </span>
        </div>

        <div className="listing-card__row">
          <span className="listing-card__label">Price:</span>
          <span className="listing-card__value">
            {listing.price_usdc > 0 ? `${listing.price_usdc} USDC` : "No minimum"}
          </span>
        </div>

        {hasActiveSwap && (
          <div className="listing-card__swap-info">
            <div className="listing-card__swap-header">Active Swap</div>
            <div className="listing-card__swap-row">
              <span className="listing-card__label">Buyer:</span>
              <span className="listing-card__value">
                {listing.active_swap.buyer.substring(0, 10)}...
              </span>
            </div>
            <div className="listing-card__swap-row">
              <span className="listing-card__label">Amount:</span>
              <span className="listing-card__value">
                {listing.active_swap.usdc_amount} USDC
              </span>
            </div>
          </div>
        )}
      </div>

      {hasActiveSwap && wallet && (
        <div className="listing-card__action">
          <ConfirmSwapForm
            swap={listing.active_swap}
            wallet={wallet}
            onSuccess={onSwapUpdated}
          />
        </div>
      )}

      {!hasActiveSwap && (
        <div className="listing-card__empty-action">
          <p>No pending swaps for this listing</p>
        </div>
      )}
    </div>
  );
}

/**
 * MyListingsDashboard
 *
 * Seller-facing page that lists all IP assets registered by the connected wallet.
 * For each listing, shows any active pending swap and allows confirming it.
 * Polls every 15 s and exposes a manual refresh button.
 *
 * Renders nothing when no wallet is connected — the WalletConnectButton
 * in the header handles that prompt.
 */
export function MyListingsDashboard() {
  const { wallet } = useWallet();
  const { listings, loading, error, refresh } = useMyListings(
    wallet?.address ?? null
  );
  const [showRegisterForm, setShowRegisterForm] = useState(false);

  const handleRegisterSuccess = () => {
    setShowRegisterForm(false);
    refresh();
  };

  // ── Not connected ──────────────────────────────────────────────────────────
  if (!wallet) {
    return (
      <section className="mld" aria-label="My Listings Dashboard">
        <div className="mld__empty mld__empty--disconnected">
          <span className="mld__empty-icon" aria-hidden="true">🔌</span>
          <p>Connect your wallet to view your listings.</p>
        </div>
      </section>
    );
  }

  // ── Connected ──────────────────────────────────────────────────────────────
  const listingsWithSwaps = listings.filter((l) => l.active_swap);
  const listingsWithoutSwaps = listings.filter((l) => !l.active_swap);

  return (
    <section className="mld" aria-label="My Listings Dashboard">
      <div className="mld__header">
        <h2 className="mld__title">My Listings</h2>
        <div className="mld__header-actions">
          <button
            className="mld__register-btn"
            onClick={() => setShowRegisterForm(true)}
            aria-label="Register new IP listing"
          >
            <span aria-hidden="true">+</span>
            Register New IP
          </button>
          <button
            className="mld__refresh-btn"
            onClick={refresh}
            disabled={loading}
            aria-label="Refresh listings"
            aria-busy={loading}
          >
            {loading ? (
              <span className="mld__spinner" aria-hidden="true" />
            ) : (
              <span aria-hidden="true">↻</span>
            )}
            {loading ? "Loading…" : "Refresh"}
          </button>
        </div>
      </div>

      {/* Register IP Form Modal */}
      {showRegisterForm && (
        <div className="mld__modal-overlay" onClick={() => setShowRegisterForm(false)}>
          <div className="mld__modal-content" onClick={(e) => e.stopPropagation()}>
            <RegisterListingForm
              wallet={wallet}
              onSuccess={handleRegisterSuccess}
              onCancel={() => setShowRegisterForm(false)}
            />
          </div>
        </div>
      )}

      {error && (
        <p className="mld__error" role="alert">
          {error}
        </p>
      )}

      {/* Initial skeleton while loading for the first time */}
      {loading && listings.length === 0 && (
        <ul className="mld__list" aria-label="Loading listings">
          {[1, 2, 3].map((n) => (
            <li key={n} className="mld__skeleton" aria-hidden="true" />
          ))}
        </ul>
      )}

      {/* Empty state */}
      {!loading && listings.length === 0 && !error && (
        <div className="mld__empty">
          <span className="mld__empty-icon" aria-hidden="true">📋</span>
          <p>No listings found for this wallet.</p>
        </div>
      )}

      {/* Listings with active swaps (highest priority) */}
      {listingsWithSwaps.length > 0 && (
        <div className="mld__section">
          <h3 className="mld__section-title">Pending Confirmations</h3>
          <ul className="mld__list">
            {listingsWithSwaps.map((listing) => (
              <li key={listing.id}>
                <ListingCard
                  listing={listing}
                  wallet={wallet}
                  onSwapUpdated={refresh}
                />
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Listings without active swaps (secondary section) */}
      {listingsWithoutSwaps.length > 0 && (
        <div className="mld__section">
          <h3 className="mld__section-title">All Listings</h3>
          <ul className="mld__list">
            {listingsWithoutSwaps.map((listing) => (
              <li key={listing.id}>
                <ListingCard
                  listing={listing}
                  wallet={wallet}
                  onSwapUpdated={refresh}
                />
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
