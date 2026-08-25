import { useEffect, useRef } from "react";
import { useCountdown } from "../hooks/useCountdown";
import "./HoldPeriodDisplay.css";

interface HoldPeriodDisplayProps {
  /** Unix timestamp (secs) when the hold ends, or null when no hold applies. */
  holdUntil: number | null;
  /** True once the buyer has confirmed receipt (waives the remaining hold). */
  buyerConfirmed: boolean;
  /** Swap status string from the contract (e.g. "Completed", "ResolvedSeller"). */
  status: string;
  /** On-chain ledger timestamp (secs), as fetched via getLedgerTimestamp(). */
  ledgerTimestamp: number;
  /** Optional callback fired once the hold period expires. */
  onExpired?: () => void;
}

function formatTimeRemaining(seconds: number): string {
  if (seconds <= 0) return "0s";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  if (hours > 0) return `${hours}h ${minutes}m ${secs}s`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

/**
 * Shows the escrow hold-period status for a completed swap:
 *  - "Held" with a live countdown while funds are time-locked,
 *  - "Confirmed by buyer" when the buyer has released the hold early,
 *  - "Hold elapsed" once the period has passed (funds are releasable).
 *
 * Renders nothing when the swap has no hold or is no longer in escrow.
 */
export function HoldPeriodDisplay({
  holdUntil,
  buyerConfirmed,
  status,
  ledgerTimestamp,
  onExpired,
}: HoldPeriodDisplayProps) {
  // useCountdown ticks the displayed timer locally between polls; elapsed
  // status is decided against ledgerTimestamp (the on-chain source of truth)
  // so a skewed local clock can't misreport whether funds are releasable.
  const { remaining } = useCountdown(holdUntil ?? 0);
  const isElapsed =
    holdUntil !== null && (ledgerTimestamp >= holdUntil || remaining === 0);

  const notifiedRef = useRef(false);
  useEffect(() => {
    if (isElapsed) {
      if (!notifiedRef.current) {
        notifiedRef.current = true;
        onExpired?.();
      }
    } else {
      notifiedRef.current = false;
    }
  }, [isElapsed, onExpired]);

  // No hold configured for this swap, or it has already settled.
  if (holdUntil === null || status !== "Completed") return null;

  if (buyerConfirmed) {
    return (
      <div className="hold-period hold-period--released" role="status">
        <span className="hold-period__label">Escrow hold</span>
        <span className="hold-period__state">Released — buyer confirmed</span>
      </div>
    );
  }

  if (isElapsed) {
    return (
      <div className="hold-period hold-period--elapsed" role="status">
        <span className="hold-period__label">Escrow hold</span>
        <span className="hold-period__state">Elapsed — funds releasable</span>
      </div>
    );
  }

  return (
    <div
      className="hold-period hold-period--active"
      role="timer"
      aria-label={`Escrow hold releases in ${formatTimeRemaining(remaining)}`}
    >
      <span className="hold-period__label">Escrow hold</span>
      <span className="hold-period__time">{formatTimeRemaining(remaining)}</span>
    </div>
  );
}
