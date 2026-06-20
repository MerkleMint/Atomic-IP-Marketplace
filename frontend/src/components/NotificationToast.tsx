import { useEffect, useRef, useState } from "react";
import type { AppNotification } from "../services/notificationService";
import { useNotifications } from "../context/NotificationContext";
import "./NotificationToast.css";

const TOAST_DURATION_MS = 5000;
const MAX_VISIBLE_TOASTS = 3;

/**
 * NotificationToast
 *
 * Renders a stack of in-app toast notifications in the bottom-right corner.
 * Automatically dismisses after TOAST_DURATION_MS.
 * Supports manual dismiss and click-through navigation.
 */
export function NotificationToast() {
  const { notifications, markRead } = useNotifications();

  // Only show the most recent unread notifications as toasts
  const [toastQueue, setToastQueue] = useState<AppNotification[]>([]);
  const seenIds = useRef(new Set<string>());

  useEffect(() => {
    const fresh = notifications.filter(
      (n) => !n.read && !seenIds.current.has(n.id)
    );
    if (fresh.length === 0) return;

    for (const n of fresh) {
      seenIds.current.add(n.id);
    }

    setToastQueue((prev) => [...fresh, ...prev].slice(0, MAX_VISIBLE_TOASTS));
  }, [notifications]);

  const dismiss = (id: string) => {
    markRead(id);
    setToastQueue((prev) => prev.filter((t) => t.id !== id));
  };

  if (toastQueue.length === 0) return null;

  return (
    <div
      className="nt-container"
      role="region"
      aria-label="Notifications"
      aria-live="polite"
      aria-atomic="false"
    >
      {toastQueue.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={dismiss} />
      ))}
    </div>
  );
}

interface ToastItemProps {
  toast: AppNotification;
  onDismiss: (id: string) => void;
}

function ToastItem({ toast, onDismiss }: ToastItemProps) {
  const [exiting, setExiting] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const startExit = () => {
    setExiting(true);
    setTimeout(() => onDismiss(toast.id), 300); // match CSS transition
  };

  useEffect(() => {
    timerRef.current = setTimeout(startExit, TOAST_DURATION_MS);
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const icon = severityIcon(toast.severity);

  const content = (
    <div
      className={`nt-toast nt-toast--${toast.severity} ${exiting ? "nt-toast--exit" : ""}`}
      role="alert"
      aria-live="assertive"
    >
      <span className="nt-toast__icon" aria-hidden="true">
        {icon}
      </span>
      <div className="nt-toast__body">
        <p className="nt-toast__title">{toast.title}</p>
        <p className="nt-toast__message">{toast.message}</p>
      </div>
      <button
        className="nt-toast__close"
        onClick={(e) => {
          e.stopPropagation();
          startExit();
        }}
        aria-label="Dismiss notification"
      >
        ×
      </button>
    </div>
  );

  if (toast.href) {
    return (
      <a
        href={toast.href}
        className="nt-toast-link"
        onClick={() => onDismiss(toast.id)}
        aria-label={`${toast.title} — click to view`}
      >
        {content}
      </a>
    );
  }

  return content;
}

function severityIcon(severity: AppNotification["severity"]): string {
  switch (severity) {
    case "success":
      return "✓";
    case "warning":
      return "⚠";
    case "error":
      return "✕";
    default:
      return "ℹ";
  }
}
