import { useWallet } from "../context/WalletContext";
import { useMyListings } from "../hooks/useMyListings";
import { ListingCard } from "./ListingCard";
import "./MyListingsDashboard.css";

/**
 * MyListingsDashboard
 *
 * Seller-facing page that shows all IP listings registered by the connected
 * wallet, along with any pending swaps that require confirmation.
 *
 * Polls every 15 s and exposes a manual refresh button.
 */
export function MyListingsDashboard() {
  const { wallet } = useWallet();
  const { listings, loading, error, refresh } = useMyListings(
    wallet?.address ?? null,
  );

  if (!wallet) {
    return (
      <section className="mld" aria-label="My Listings Dashboard">
        <div className="mld__empty mld__empty--disconnected">
          <span className="mld__empty-icon" aria-hidden="true">
            🔌
          </span>
          <p>Connect your wallet to view your listings.</p>
        </div>
      </section>
    );
  }

  const withPending = listings.filter((l) => l.pendingSwaps.length > 0);
  const withoutPending = listings.filter((l) => l.pendingSwaps.length === 0);

  return (
    <section className="mld" aria-label="My Listings Dashboard">
      <div className="mld__header">
        <h2 className="mld__title">My Listings</h2>
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

      {/* Skeleton while loading for the first time */}
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
          <span className="mld__empty-icon" aria-hidden="true">
            📂
          </span>
          <p>No listings found for this wallet.</p>
        </div>
      )}

      {/* Listings with pending swaps first */}
      {withPending.length > 0 && (
        <div className="mld__group">
          <h3 className="mld__group-title">
            Action Required
            <span className="mld__badge">{withPending.length}</span>
          </h3>
          <ul className="mld__list">
            {withPending.map((listing) => (
              <li key={listing.id}>
                <ListingCard
                  listing={listing}
                  wallet={wallet}
                  onUpdated={refresh}
                />
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Listings without pending swaps */}
      {withoutPending.length > 0 && (
        <div className="mld__group">
          <h3 className="mld__group-title">
            All Listings
            <span className="mld__badge mld__badge--muted">
              {withoutPending.length}
            </span>
          </h3>
          <ul className="mld__list">
            {withoutPending.map((listing) => (
              <li key={listing.id}>
                <ListingCard
                  listing={listing}
                  wallet={wallet}
                  onUpdated={refresh}
                />
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
