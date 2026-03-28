import React from "react";
import { useWallet } from "../context/WalletContext";
import { useMySwaps } from "../hooks/useMySwaps";
import { SwapCard } from "./SwapCard";
import "./MySwapsDashboard.css";

/**
 * MySwapsDashboard
 *
 * Buyer-facing page that lists all swaps associated with the connected wallet.
 * Polls every 15 s and exposes a manual refresh button.
 *
 * Renders nothing when no wallet is connected — the WalletConnectButton
 * in the header handles that prompt.
 */
export function MySwapsDashboard() {
  const { wallet } = useWallet();
  const { swaps, ledgerTimestamp, loading, error, refresh } = useMySwaps(
    wallet?.address ?? null
  );

  // ── Not connected ──────────────────────────────────────────────────────────
  if (!wallet) {
    return (
      <section className="msd" aria-label="My Swaps Dashboard">
        <div className="msd__empty msd__empty--disconnected">
          <span className="msd__empty-icon" aria-hidden="true">🔌</span>
          <p>Connect your wallet to view your swaps.</p>
        </div>
      </section>
    );
  }

  // ── Connected ──────────────────────────────────────────────────────────────
  const pendingSwaps = swaps.filter((s) => s.status === "Pending");
  const settledSwaps = swaps.filter((s) => s.status !== "Pending");

  return (
    <section className="msd" aria-label="My Swaps Dashboard">
      <div className="msd__header">
        <h2 className="msd__title">My Swaps</h2>
        <button
          className="msd__refresh-btn"
          onClick={refresh}
          disabled={loading}
          aria-label="Refresh swaps"
          aria-busy={loading}
        >
          {loading ? (
            <span className="msd__spinner" aria-hidden="true" />
          ) : (
            <span aria-hidden="true">↻</span>
          )}
          {loading ? "Loading…" : "Refresh"}
        </button>
      </div>

      {error && (
        <p className="msd__error" role="alert">
          {error}
        </p>
      )}

      {/* Initial skeleton while loading for the first time */}
      {loading && swaps.length === 0 && (
        <ul className="msd__list" aria-label="Loading swaps">
          {[1, 2, 3].map((n) => (
            <li key={n} className="msd__skeleton" aria-hidden="true" />
          ))}
        </ul>
      )}

      {/* Empty state */}
      {!loading && swaps.length === 0 && !error && (
        <div className="msd__empty">
          <span className="msd__empty-icon" aria-hidden="true">📭</span>
          <p>No swaps found for this wallet.</p>
        </div>
      )}

      {/* Active / pending swaps */}
      {pendingSwaps.length > 0 && (
        <div className="msd__group">
          <h3 className="msd__group-title">
            Active
            <span className="msd__badge">{pendingSwaps.length}</span>
          </h3>
          <ul className="msd__list">
            {pendingSwaps.map((swap) => (
              <li key={swap.id}>
                <SwapCard
                  swap={swap}
                  ledgerTimestamp={ledgerTimestamp}
                  wallet={wallet}
                  onSwapUpdated={refresh}
                />
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Completed / cancelled swaps */}
      {settledSwaps.length > 0 && (
        <div className="msd__group">
          <h3 className="msd__group-title">
            History
            <span className="msd__badge msd__badge--muted">{settledSwaps.length}</span>
          </h3>
          <ul className="msd__list">
            {settledSwaps.map((swap) => (
              <li key={swap.id}>
                <SwapCard
                  swap={swap}
                  ledgerTimestamp={ledgerTimestamp}
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
