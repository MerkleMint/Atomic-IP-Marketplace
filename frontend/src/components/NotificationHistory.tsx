import { useState } from "react";
import { useNotifications } from "../context/NotificationContext";
import type { AppNotification } from "../services/notificationService";
import { NotificationPreferences } from "./NotificationPreferences";
import "./NotificationHistory.css";

interface Props {
  onClose: () => void;
}

type Tab = "inbox" | "archive" | "settings";

/**
 * NotificationHistory
 *
 * Three-tab panel rendered inside NotificationBell's dropdown:
 *  - Inbox: unread + read (non-archived)
 *  - Archive: archived entries
 *  - Settings: NotificationPreferences UI
 */
export function NotificationHistory({ onClose }: Props) {
  const {
    notifications,
    unreadCount,
    getFullHistory,
    markRead,
    markAllRead,
    archive,
    archiveAll,
    clearHistory,
  } = useNotifications();

  const [tab, setTab] = useState<Tab>("inbox");

  const archived = getFullHistory().filter((n) => n.archived);
  const inbox = notifications; // already excludes archived

  return (
    <div className="nh">
      {/* Header */}
      <div className="nh__header">
        <h3 className="nh__title">Notifications</h3>
        <button className="nh__close" onClick={onClose} aria-label="Close notifications">
          ×
        </button>
      </div>

      {/* Tabs */}
      <div className="nh__tabs" role="tablist">
        <button
          className={`nh__tab ${tab === "inbox" ? "nh__tab--active" : ""}`}
          role="tab"
          aria-selected={tab === "inbox"}
          onClick={() => setTab("inbox")}
        >
          Inbox
          {unreadCount > 0 && (
            <span className="nh__tab-badge">{unreadCount > 99 ? "99+" : unreadCount}</span>
          )}
        </button>
        <button
          className={`nh__tab ${tab === "archive" ? "nh__tab--active" : ""}`}
          role="tab"
          aria-selected={tab === "archive"}
          onClick={() => setTab("archive")}
        >
          Archive
          {archived.length > 0 && (
            <span className="nh__tab-badge nh__tab-badge--muted">{archived.length}</span>
          )}
        </button>
        <button
          className={`nh__tab ${tab === "settings" ? "nh__tab--active" : ""}`}
          role="tab"
          aria-selected={tab === "settings"}
          onClick={() => setTab("settings")}
        >
          Settings
        </button>
      </div>

      {/* Tab panels */}
      {tab === "inbox" && (
        <InboxPanel
          items={inbox}
          onMarkRead={markRead}
          onMarkAllRead={markAllRead}
          onArchive={archive}
          onArchiveAll={archiveAll}
        />
      )}

      {tab === "archive" && (
        <ArchivePanel
          items={archived}
          onClear={clearHistory}
        />
      )}

      {tab === "settings" && <NotificationPreferences />}
    </div>
  );
}

// ─── Inbox panel ───────────────────────────────────────────────────────────────

interface InboxPanelProps {
  items: AppNotification[];
  onMarkRead: (id: string) => void;
  onMarkAllRead: () => void;
  onArchive: (id: string) => void;
  onArchiveAll: () => void;
}

function InboxPanel({ items, onMarkRead, onMarkAllRead, onArchive, onArchiveAll }: InboxPanelProps) {
  const hasUnread = items.some((n) => !n.read);

  return (
    <div className="nh__panel" role="tabpanel">
      {items.length > 0 && (
        <div className="nh__actions">
          {hasUnread && (
            <button className="nh__action-btn" onClick={onMarkAllRead}>
              Mark all read
            </button>
          )}
          <button className="nh__action-btn" onClick={onArchiveAll}>
            Archive all
          </button>
        </div>
      )}

      {items.length === 0 ? (
        <div className="nh__empty">
          <span className="nh__empty-icon" aria-hidden="true">🔔</span>
          <p>No notifications yet.</p>
        </div>
      ) : (
        <ul className="nh__list" role="list">
          {items.map((n) => (
            <NotificationItem
              key={n.id}
              notification={n}
              onMarkRead={onMarkRead}
              onArchive={onArchive}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

// ─── Archive panel ─────────────────────────────────────────────────────────────

interface ArchivePanelProps {
  items: AppNotification[];
  onClear: () => void;
}

function ArchivePanel({ items, onClear }: ArchivePanelProps) {
  return (
    <div className="nh__panel" role="tabpanel">
      {items.length > 0 && (
        <div className="nh__actions">
          <button className="nh__action-btn nh__action-btn--danger" onClick={onClear}>
            Clear all history
          </button>
        </div>
      )}

      {items.length === 0 ? (
        <div className="nh__empty">
          <span className="nh__empty-icon" aria-hidden="true">📂</span>
          <p>Archive is empty.</p>
        </div>
      ) : (
        <ul className="nh__list" role="list">
          {items.map((n) => (
            <NotificationItem key={n.id} notification={n} archived />
          ))}
        </ul>
      )}
    </div>
  );
}

// ─── Single notification row ───────────────────────────────────────────────────

interface NotificationItemProps {
  notification: AppNotification;
  onMarkRead?: (id: string) => void;
  onArchive?: (id: string) => void;
  archived?: boolean;
}

function NotificationItem({ notification: n, onMarkRead, onArchive, archived }: NotificationItemProps) {
  const timeAgo = formatTimeAgo(n.timestamp);

  const inner = (
    <li
      className={`nh__item nh__item--${n.severity} ${!n.read ? "nh__item--unread" : ""}`}
      onClick={() => onMarkRead?.(n.id)}
    >
      <span className="nh__item-dot" aria-hidden="true" />
      <div className="nh__item-body">
        <div className="nh__item-header">
          <span className="nh__item-title">{n.title}</span>
          <span className="nh__item-time">{timeAgo}</span>
        </div>
        <p className="nh__item-message">{n.message}</p>
      </div>
      {!archived && onArchive && (
        <button
          className="nh__item-archive"
          onClick={(e) => { e.stopPropagation(); onArchive(n.id); }}
          aria-label="Archive notification"
          title="Archive"
        >
          ↓
        </button>
      )}
    </li>
  );

  if (n.href) {
    return (
      <a href={n.href} className="nh__item-link" onClick={() => onMarkRead?.(n.id)}>
        {inner}
      </a>
    );
  }

  return inner;
}

// ─── Helpers ───────────────────────────────────────────────────────────────────

function formatTimeAgo(timestamp: number): string {
  const diff = Date.now() - timestamp;
  const s = Math.floor(diff / 1000);
  if (s < 60) return "just now";
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  return `${d}d ago`;
}
