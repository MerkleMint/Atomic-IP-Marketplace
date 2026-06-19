import { useEffect, useState } from "react";
import "./HoldPeriodDisplay.css";

interface HoldPeriodDisplayProps {
  /** Unix timestamp (secs) when the hold ends, or null when no hold applies. */
  holdUntil: number | null;
  /** True once the buyer has confirmed receipt (waives the remaining hold). */
  buyerConfirmed: boolean;
  /** Swap status string from the contract (e.g. "Completed", "ResolvedSeller"). */
  status: string;
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
  onExpired,
}: HoldPeriodDisplayProps) {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

  const isActive =
    status === "Completed" &&
    !buyerConfirmed &&
    holdUntil !== null &&
    now < holdUntil;

  useEffect(() => {
    if (!isActive) return;
    const id = setInterval(() => {
      const t = Math.floor(Date.now() / 1000);
      setNow(t);
      if (holdUntil !== null && t >= holdUntil) {
        onExpired?.();
      }
    }, 1000);
    return () => clearInterval(id);
  }, [isActive, holdUntil, onExpired]);

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

  const remaining = Math.max(0, holdUntil - now);
  if (remaining <= 0) {
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
