import { useState } from "react";
import { useCountdown } from "../hooks/useCountdown";
import { cancelSwap } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./CancelSwapButton.css";

interface Props {
  swap: Swap;
  ledgerTimestamp: number;
  wallet: Wallet;
  onSuccess: () => void;
}

export function CancelSwapButton({ swap, ledgerTimestamp, wallet, onSuccess }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { remaining, formatted } = useCountdown(swap.expires_at);

  if (swap.status !== "Pending") return null;

  const isExpired = ledgerTimestamp >= swap.expires_at || remaining === 0;

  const handleCancel = async () => {
    setError(null);
    setLoading(true);
    try {
      await cancelSwap(swap.id, wallet);
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to cancel swap.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="cancel-swap-wrapper">
      {isExpired ? (
        <button className="cancel-swap-btn" onClick={handleCancel} disabled={loading} aria-busy={loading}>
          {loading && <span className="cancel-swap-spinner" aria-hidden="true" />}
          {loading ? "Cancelling…" : "Cancel Swap"}
        </button>
      ) : (
        <div className="cancel-swap-countdown" aria-label="Time until cancellable">
          <span className="cancel-swap-countdown__label">Cancellable in</span>
          <span className="cancel-swap-countdown__timer">{formatted}</span>
        </div>
      )}
      {error && <p className="cancel-swap-error" role="alert">{error}</p>}
    </div>
  );
}
