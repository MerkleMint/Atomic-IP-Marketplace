import { useEffect, useState } from "react";
import "./DisputeCountdown.css";

interface DisputeCountdownProps {
  expiresAt: number;
  currentTime: number;
  onExpired?: () => void;
}

function formatTimeRemaining(seconds: number): string {
  if (seconds <= 0) return "Expired";

  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${secs}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  }
  return `${secs}s`;
}

export function DisputeCountdown({
  expiresAt,
  currentTime,
  onExpired,
}: DisputeCountdownProps) {
  const [timeRemaining, setTimeRemaining] = useState(() =>
    Math.max(0, expiresAt - currentTime)
  );
  const [isExpired, setIsExpired] = useState(timeRemaining <= 0);

  useEffect(() => {
    let frame: number;

    const updateCountdown = () => {
      const now = Math.floor(Date.now() / 1000);
      const remaining = Math.max(0, expiresAt - now);
      setTimeRemaining(remaining);

      if (remaining <= 0 && !isExpired) {
        setIsExpired(true);
        onExpired?.();
      } else {
        frame = requestAnimationFrame(updateCountdown);
      }
    };

    frame = requestAnimationFrame(updateCountdown);

    return () => cancelAnimationFrame(frame);
  }, [expiresAt, isExpired, onExpired]);

  const isUrgent = timeRemaining < 3600; // Less than 1 hour
  const isCritical = timeRemaining < 600; // Less than 10 minutes

  return (
    <div
      className={`dispute-countdown ${
        isCritical ? "dispute-countdown--critical" : isUrgent ? "dispute-countdown--urgent" : ""
      }`}
      role="timer"
      aria-label={`Dispute resolution expires in ${formatTimeRemaining(timeRemaining)}`}
    >
      <span className="dispute-countdown__label">Expires</span>
      <span className="dispute-countdown__time">{formatTimeRemaining(timeRemaining)}</span>
      {isCritical && (
        <span className="dispute-countdown__warning" aria-label="Critical - expiring soon">
          ⚠
        </span>
      )}
    </div>
  );
}
