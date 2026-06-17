import "./SwapStatusBadge.css";

interface SwapStatusBadgeProps {
  status: "Pending" | "Completed" | "Cancelled" | "Disputed";
  confirmations?: number;
  targetConfirmations?: number;
}

const statusConfig = {
  Pending: {
    label: "Pending",
    color: "status-badge--pending",
    icon: "⏳",
  },
  Completed: {
    label: "Completed",
    color: "status-badge--completed",
    icon: "✓",
  },
  Cancelled: {
    label: "Cancelled",
    color: "status-badge--cancelled",
    icon: "✕",
  },
  Disputed: {
    label: "Disputed",
    color: "status-badge--disputed",
    icon: "⚠",
  },
};

export function SwapStatusBadge({
  status,
  confirmations,
  targetConfirmations = 3,
}: SwapStatusBadgeProps) {
  const config = statusConfig[status];
  const confirmationPercent = confirmations
    ? Math.round((confirmations / targetConfirmations) * 100)
    : 0;

  return (
    <div
      className={`status-badge ${config.color}`}
      aria-label={`Status: ${config.label}`}
      title={
        confirmations
          ? `${confirmations}/${targetConfirmations} confirmations`
          : config.label
      }
    >
      <span className="status-badge__icon" aria-hidden="true">
        {config.icon}
      </span>
      <span className="status-badge__label">{config.label}</span>

      {confirmations !== undefined && status === "Pending" && (
        <div
          className="status-badge__progress"
          style={{
            width: `${Math.min(confirmationPercent, 100)}%`,
          }}
          role="progressbar"
          aria-valuenow={confirmations}
          aria-valuemin={0}
          aria-valuemax={targetConfirmations}
        />
      )}
    </div>
  );
}
