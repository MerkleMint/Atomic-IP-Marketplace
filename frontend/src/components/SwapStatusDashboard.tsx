import { useState, useEffect, useRef } from "react";
import { useWallet } from "../context/WalletContext";
import { useMySwaps, Swap } from "../hooks/useMySwaps";
import { SwapStatusBadge } from "./SwapStatusBadge";
import { DisputeCountdown } from "./DisputeCountdown";
import { SwapHistoryTable } from "./SwapHistoryTable";
import { pollingService } from "../services/contractPollingService";
import { getSwap } from "../lib/contractClient";
import "./SwapStatusDashboard.css";

interface SwapWithPolling extends Swap {
  confirmations?: number;
  isPolling?: boolean;
  pollError?: string;
}

export function SwapStatusDashboard() {
  const { wallet } = useWallet();
  const { swaps: initialSwaps, ledgerTimestamp, loading, error, refresh } =
    useMySwaps(wallet?.address ?? null);

  const [swaps, setSwaps] = useState<SwapWithPolling[]>([]);
  const [activeSwapPolls, setActiveSwapPolls] = useState(new Set<number>());
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [view, setView] = useState<"active" | "history">("active");
  const visibilityRef = useRef<boolean>(true);

  // Map initial swaps data
  useEffect(() => {
    const mapped = initialSwaps.map((swap) => ({
      ...swap,
      isPolling: swap.status === "Pending",
    }));
    setSwaps(mapped);
  }, [initialSwaps]);

  // Start polling for pending swaps with error boundary
  useEffect(() => {
    const pendingSwapIds = swaps
      .filter((s) => s.status === "Pending")
      .map((s) => s.id);

    for (const swapId of pendingSwapIds) {
      if (!activeSwapPolls.has(swapId)) {
        setActiveSwapPolls((prev) => new Set([...prev, swapId]));

        pollingService.poll<Swap | null>(
          `swap_${swapId}`,
          () => getSwap(swapId),
          (updatedSwap) => {
            if (updatedSwap) {
              setSwaps((prev) =>
                prev.map((s) =>
                  s.id === swapId
                    ? {
                        ...updatedSwap,
                        isPolling: updatedSwap.status === "Pending",
                        pollError: undefined,
                      }
                    : s
                )
              );
              setConnectionError(null);
            }
          },
          (err) => {
            setConnectionError(err.message);
            setSwaps((prev) =>
              prev.map((s) =>
                s.id === swapId
                  ? { ...s, pollError: err.message }
                  : s
              )
            );
          }
        );
      }
    }

    // Clean up polling for completed swaps
    for (const swapId of activeSwapPolls) {
      if (!pendingSwapIds.includes(swapId)) {
        pollingService.stop(`swap_${swapId}`);
        setActiveSwapPolls((prev) => {
          const next = new Set(prev);
          next.delete(swapId);
          return next;
        });
      }
    }
  }, [swaps, activeSwapPolls]);

  // Handle visibility change for graceful polling pause
  useEffect(() => {
    const handleVisibilityChange = () => {
      visibilityRef.current = !document.hidden;

      if (document.hidden) {
        pollingService.stopAll();
      } else {
        pollingService.resume();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);
    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, []);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      pollingService.stopAll();
    };
  }, []);

  if (!wallet) {
    return (
      <section className="swap-status-dashboard" aria-label="Swap Status Dashboard">
        <div className="swap-status-dashboard__empty">
          <span className="swap-status-dashboard__empty-icon" aria-hidden="true">
            🔌
          </span>
          <p>Connect your wallet to view swap status.</p>
        </div>
      </section>
    );
  }

  const pendingSwaps = swaps.filter((s) => s.status === "Pending");
  const historySwaps = swaps.filter((s) => s.status !== "Pending");

  return (
    <section
      className="swap-status-dashboard"
      aria-label="Swap Status Dashboard"
    >
      {/* Header */}
      <div className="swap-status-dashboard__header">
        <div>
          <h1 className="swap-status-dashboard__title">Swap Status Dashboard</h1>
          <p className="swap-status-dashboard__subtitle">
            Real-time transaction tracking with blockchain confirmations
          </p>
        </div>
        <button
          className={`swap-status-dashboard__refresh ${
            loading ? "swap-status-dashboard__refresh--loading" : ""
          }`}
          onClick={refresh}
          disabled={loading}
          aria-busy={loading}
          aria-label={loading ? "Refreshing..." : "Refresh swap data"}
        >
          {loading ? (
            <>
              <span
                className="swap-status-dashboard__spinner"
                aria-hidden="true"
              />
              Loading…
            </>
          ) : (
            <>
              <span aria-hidden="true">↻</span>
              Refresh
            </>
          )}
        </button>
      </div>

      {/* Connection Status */}
      {connectionError && (
        <div
          className="swap-status-dashboard__connection-error"
          role="alert"
          aria-live="polite"
        >
          <span aria-hidden="true">⚠</span>
          <div>
            <strong>Connection Issue</strong>
            <p>{connectionError}</p>
          </div>
        </div>
      )}

      {error && (
        <div className="swap-status-dashboard__error" role="alert">
          {error}
        </div>
      )}

      {/* View Tabs */}
      <div className="swap-status-dashboard__tabs" role="tablist">
        <button
          role="tab"
          aria-selected={view === "active"}
          aria-controls="active-panel"
          className={`swap-status-dashboard__tab ${
            view === "active"
              ? "swap-status-dashboard__tab--active"
              : ""
          }`}
          onClick={() => setView("active")}
        >
          <span aria-hidden="true">⏳</span>
          Active Swaps{" "}
          {pendingSwaps.length > 0 && (
            <span className="swap-status-dashboard__tab-badge">
              {pendingSwaps.length}
            </span>
          )}
        </button>
        <button
          role="tab"
          aria-selected={view === "history"}
          aria-controls="history-panel"
          className={`swap-status-dashboard__tab ${
            view === "history"
              ? "swap-status-dashboard__tab--active"
              : ""
          }`}
          onClick={() => setView("history")}
        >
          <span aria-hidden="true">📋</span>
          Transaction History{" "}
          {historySwaps.length > 0 && (
            <span className="swap-status-dashboard__tab-badge">
              {historySwaps.length}
            </span>
          )}
        </button>
      </div>

      {/* Active Swaps Panel */}
      {view === "active" && (
        <div
          id="active-panel"
          role="tabpanel"
          className="swap-status-dashboard__panel"
        >
          {pendingSwaps.length === 0 ? (
            <div className="swap-status-dashboard__empty">
              <span className="swap-status-dashboard__empty-icon" aria-hidden="true">
                ✓
              </span>
              <p>No active swaps. All transactions completed!</p>
            </div>
          ) : (
            <div className="swap-status-dashboard__grid">
              {pendingSwaps.map((swap) => (
                <article
                  key={swap.id}
                  className="swap-status-dashboard__card"
                  aria-label={`Swap ${swap.id}`}
                >
                  <div className="swap-status-dashboard__card-header">
                    <h3 className="swap-status-dashboard__card-title">
                      Swap #{swap.id}
                    </h3>
                    <SwapStatusBadge
                      status={swap.status as any}
                      confirmations={swap.confirmations}
                    />
                  </div>

                  <div className="swap-status-dashboard__card-content">
                    <div className="swap-status-dashboard__field">
                      <span className="swap-status-dashboard__label">
                        Amount
                      </span>
                      <span className="swap-status-dashboard__value">
                        {(swap.usdc_amount / 1e7).toFixed(2)} USDC
                      </span>
                    </div>

                    <div className="swap-status-dashboard__field">
                      <span className="swap-status-dashboard__label">
                        Parties
                      </span>
                      <span className="swap-status-dashboard__value swap-status-dashboard__value--code">
                        {swap.buyer === wallet.address ? "You are buyer" : "You are seller"}
                      </span>
                    </div>

                    <div className="swap-status-dashboard__field">
                      <span className="swap-status-dashboard__label">
                        Created
                      </span>
                      <span className="swap-status-dashboard__value">
                        {new Date(swap.created_at * 1000).toLocaleString()}
                      </span>
                    </div>

                    {swap.expires_at > 0 && (
                      <div className="swap-status-dashboard__field">
                        <DisputeCountdown
                          expiresAt={swap.expires_at}
                          currentTime={ledgerTimestamp}
                        />
                      </div>
                    )}

                    {swap.pollError && (
                      <div
                        className="swap-status-dashboard__poll-error"
                        role="alert"
                      >
                        Failed to update: {swap.pollError}
                      </div>
                    )}
                  </div>

                  <div className="swap-status-dashboard__card-footer">
                    <button
                      className="swap-status-dashboard__card-action"
                      aria-label={`View full details for swap ${swap.id}`}
                    >
                      View Details
                    </button>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      )}

      {/* History Panel */}
      {view === "history" && (
        <div
          id="history-panel"
          role="tabpanel"
          className="swap-status-dashboard__panel"
        >
          <SwapHistoryTable
            swaps={swaps}
            pageSize={10}
            currentPage={1}
          />
        </div>
      )}
    </section>
  );
}
