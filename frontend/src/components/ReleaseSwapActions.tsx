import { useState } from "react";
import { releaseToSeller, confirmReceipt } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./ReleaseSwapActions.css";

interface Props {
  swap: Swap;
  ledgerTimestamp: number;
  wallet: Wallet;
  onSuccess: () => void;
}

/**
 * Seller-facing "Release Payment" and buyer-facing "Confirm Receipt & Release
 * Early" actions for a completed swap. These are the only in-app path to
 * actually move the escrowed USDC to the seller — without them the funds
 * released by confirm_swap stay locked in the contract indefinitely.
 */
export function ReleaseSwapActions({ swap, ledgerTimestamp, wallet, onSuccess }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (swap.status !== "Completed") return null;

  const isBuyer = wallet.address === swap.buyer;
  const isSeller = wallet.address === swap.seller;
  if (!isBuyer && !isSeller) return null;

  const holdActive =
    swap.hold_until !== null &&
    !swap.buyer_confirmed &&
    ledgerTimestamp < swap.hold_until;

  const handleRelease = async () => {
    setError(null);
    setLoading(true);
    try {
      await releaseToSeller(swap.id, wallet);
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to release payment.");
    } finally {
      setLoading(false);
    }
  };

  const handleConfirmReceipt = async () => {
    setError(null);
    setLoading(true);
    try {
      await confirmReceipt(swap.id, wallet);
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to confirm receipt.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="release-swap-wrapper">
      {isSeller && (
        <button
          className="release-swap-btn"
          onClick={handleRelease}
          disabled={loading || holdActive}
          aria-busy={loading}
          title={holdActive ? "Escrow hold is still active." : undefined}
        >
          {loading && <span className="release-swap-spinner" aria-hidden="true" />}
          {loading ? "Releasing…" : "Release Payment"}
        </button>
      )}
      {isBuyer && !swap.buyer_confirmed && holdActive && (
        <button
          className="release-swap-btn release-swap-btn--secondary"
          onClick={handleConfirmReceipt}
          disabled={loading}
          aria-busy={loading}
        >
          {loading && <span className="release-swap-spinner" aria-hidden="true" />}
          {loading ? "Confirming…" : "Confirm Receipt & Release Early"}
        </button>
      )}
      {error && <p className="release-swap-error" role="alert">{error}</p>}
    </div>
  );
}
