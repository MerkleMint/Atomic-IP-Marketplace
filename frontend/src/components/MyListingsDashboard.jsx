import React from "react";
import { useWallet } from "../context/WalletContext";
import { useMyListings } from "../hooks/useMyListings";
import { ListingCard } from "./ListingCard";
import "./MyListingsDashboard.css";

/**
 * MyListingsDashboard
 *
 * Seller-facing page that shows all IP listings registered by the connected
 * wallet, along with any pending swaps against each listing.
 *
 * - Calls ip_registry.list_by_owner with the connected wallet address
 * - For each listing shows: IPFS hash, listing ID, and active swaps
 * - Links to the confirm swap flow for pending swaps via ConfirmSwapForm
 * - Polls every 15 s and exposes a manual refresh button
 */
export function MyListingsDashboard() {
  const { wallet } = useWallet();
  const { listings, loading, error, refresh } = useMyListings(
    wallet?.address ?? null
  );

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

  const listingsWithSwaps = listings.filter((l) => l.activeSwaps.length > 0);
  const listingsWithoutSwaps = listings.filter((l) => l.activeSwaps.length === 0);

  return (
    <section className="mld" aria-label="My Listings Dashboard">
      <div className="mld__header">
        <h2 className="mld__title">
          My Listings
          {listings.length > 0 && (
            <span className="mld__count">{listings.length}</span>
          )}
        </h2>
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
          <span className="mld__empty-icon" aria-hidden="true">📂</span>
          <p>No listings found for this wallet.</p>
        </div>
      )}

      {/* Listings with pending swaps — shown first */}
      {listingsWithSwaps.length > 0 && (
        <div className="mld__group">
          <h3 className="mld__group-title">
            Action Required
            <span className="mld__badge">{listingsWithSwaps.length}</span>
          </h3>
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

      {/* Listings without pending swaps */}
      {listingsWithoutSwaps.length > 0 && (
        <div className="mld__group">
          <h3 className="mld__group-title">
            All Listings
            <span className="mld__badge mld__badge--muted">
              {listingsWithoutSwaps.length}
            </span>
          </h3>
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
