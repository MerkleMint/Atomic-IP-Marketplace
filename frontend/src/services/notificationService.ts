/**
 * NotificationService
 *
 * Multi-channel notification service for swap events and marketplace updates.
 * Supports in-app toasts, email (via external webhook), and webhook delivery.
 *
 * Features:
 *  - Multi-channel: in-app, email, webhook
 *  - User preference management (per-channel, per-event-type opt-in/out)
 *  - Rate limiting: max 10 notifications per hour per user
 *  - Deduplication: prevents identical notifications within a 30s window
 *  - Notification history with archival (localStorage, max 200 entries)
 *  - Privacy: all delivery respects user preferences
 */

// ─── Types ─────────────────────────────────────────────────────────────────────

export type NotificationChannel = "in_app" | "email" | "webhook";

export type NotificationEventType =
  | "swap_initiated"
  | "swap_confirmed"
  | "swap_cancelled"
  | "swap_expired"
  | "swap_disputed"
  | "payment_confirmed"
  | "listing_created"
  | "listing_updated"
  | "marketplace_announcement";

export type NotificationSeverity = "info" | "success" | "warning" | "error";

export interface AppNotification {
  id: string;
  eventType: NotificationEventType;
  title: string;
  message: string;
  severity: NotificationSeverity;
  timestamp: number; // unix ms
  read: boolean;
  archived: boolean;
  /** Optional deep-link within the app (e.g. /swap/42) */
  href?: string;
  /** Contextual metadata (swap id, listing id, etc.) */
  meta?: Record<string, unknown>;
}

export interface NotificationPreferences {
  /** Which channels are enabled globally */
  channels: Record<NotificationChannel, boolean>;
  /** Per-event opt-in / opt-out. If absent, defaults to true (opted in). */
  events: Partial<Record<NotificationEventType, boolean>>;
  /** Email address for email channel */
  emailAddress?: string;
  /** Webhook URL for webhook channel */
  webhookUrl?: string;
  /** Custom webhook headers (e.g. Authorization) */
  webhookHeaders?: Record<string, string>;
}

export interface WebhookPayload {
  event: NotificationEventType;
  severity: NotificationSeverity;
  title: string;
  message: string;
  timestamp: number;
  walletAddress: string;
  meta?: Record<string, unknown>;
}

// ─── Constants ─────────────────────────────────────────────────────────────────

const STORAGE_KEY_HISTORY = "notif_history";
const STORAGE_KEY_PREFS = "notif_prefs";
const RATE_LIMIT_WINDOW_MS = 60 * 60 * 1000; // 1 hour
const RATE_LIMIT_MAX = 10;
const DEDUP_WINDOW_MS = 30_000; // 30 seconds
const MAX_HISTORY = 200;

const DEFAULT_PREFERENCES: NotificationPreferences = {
  channels: {
    in_app: true,
    email: false,
    webhook: false,
  },
  events: {},
  emailAddress: "",
  webhookUrl: "",
  webhookHeaders: {},
};

// ─── Utility ───────────────────────────────────────────────────────────────────

function generateId(): string {
  return `notif_${Date.now()}_${Math.random().toString(36).slice(2, 9)}`;
}

function dedupKey(eventType: NotificationEventType, meta?: Record<string, unknown>): string {
  const metaStr = meta ? JSON.stringify(meta) : "";
  return `${eventType}:${metaStr}`;
}

// ─── NotificationService class ─────────────────────────────────────────────────

type Listener = (notifications: AppNotification[]) => void;

export class NotificationService {
  private history: AppNotification[] = [];
  private preferences: NotificationPreferences = { ...DEFAULT_PREFERENCES };
  private listeners = new Set<Listener>();

  /** Map of dedupKey → timestamp of last delivery */
  private recentDedup = new Map<string, number>();

  /** Timestamps of notifications sent (for rate limiting), per wallet */
  private rateLimitLog: number[] = [];

  private walletAddress: string | null = null;

  constructor() {
    this.loadFromStorage();
  }

  // ─── Lifecycle ─────────────────────────────────────────────────────────────

  /** Call when wallet connects to associate notifications with user. */
  setWalletAddress(address: string | null): void {
    this.walletAddress = address;
    // Reload history / prefs scoped to this wallet
    this.loadFromStorage();
  }

  // ─── Subscription ──────────────────────────────────────────────────────────

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    // Immediately deliver current state to new subscriber
    listener([...this.history]);
    return () => this.listeners.delete(listener);
  }

  private emit(): void {
    const snapshot = [...this.history];
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }

  // ─── Core delivery ─────────────────────────────────────────────────────────

  /**
   * Send a notification through all enabled channels.
   * Handles preference filtering, rate limiting, and deduplication.
   */
  async send(
    eventType: NotificationEventType,
    title: string,
    message: string,
    severity: NotificationSeverity = "info",
    opts: { href?: string; meta?: Record<string, unknown> } = {}
  ): Promise<AppNotification | null> {
    // 1. Check per-event preference
    const eventEnabled = this.preferences.events[eventType];
    if (eventEnabled === false) return null;

    // 2. Deduplication check
    const key = dedupKey(eventType, opts.meta);
    const lastSeen = this.recentDedup.get(key);
    if (lastSeen && Date.now() - lastSeen < DEDUP_WINDOW_MS) {
      return null;
    }
    this.recentDedup.set(key, Date.now());

    // 3. Rate limiting (max 10/hour per user)
    const now = Date.now();
    this.rateLimitLog = this.rateLimitLog.filter(
      (ts) => now - ts < RATE_LIMIT_WINDOW_MS
    );
    if (this.rateLimitLog.length >= RATE_LIMIT_MAX) {
      console.warn("[NotificationService] Rate limit reached, dropping notification:", title);
      return null;
    }
    this.rateLimitLog.push(now);

    // 4. Build notification record
    const notification: AppNotification = {
      id: generateId(),
      eventType,
      title,
      message,
      severity,
      timestamp: now,
      read: false,
      archived: false,
      href: opts.href,
      meta: opts.meta,
    };

    // 5. In-app delivery
    if (this.preferences.channels.in_app) {
      this.addToHistory(notification);
      this.emit();
    }

    // 6. Email delivery (fire-and-forget, via external webhook)
    if (this.preferences.channels.email && this.preferences.emailAddress) {
      this.deliverEmail(notification).catch((err) =>
        console.error("[NotificationService] Email delivery failed:", err)
      );
    }

    // 7. Webhook delivery
    if (this.preferences.channels.webhook && this.preferences.webhookUrl) {
      this.deliverWebhook(notification).catch((err) =>
        console.error("[NotificationService] Webhook delivery failed:", err)
      );
    }

    return notification;
  }

  // ─── In-app history ────────────────────────────────────────────────────────

  private addToHistory(notification: AppNotification): void {
    this.history = [notification, ...this.history].slice(0, MAX_HISTORY);
    this.saveHistory();
  }

  getHistory(includeArchived = false): AppNotification[] {
    if (includeArchived) return [...this.history];
    return this.history.filter((n) => !n.archived);
  }

  getUnreadCount(): number {
    return this.history.filter((n) => !n.read && !n.archived).length;
  }

  markRead(id: string): void {
    const notif = this.history.find((n) => n.id === id);
    if (notif && !notif.read) {
      notif.read = true;
      this.saveHistory();
      this.emit();
    }
  }

  markAllRead(): void {
    let changed = false;
    for (const n of this.history) {
      if (!n.read) {
        n.read = true;
        changed = true;
      }
    }
    if (changed) {
      this.saveHistory();
      this.emit();
    }
  }

  archive(id: string): void {
    const notif = this.history.find((n) => n.id === id);
    if (notif && !notif.archived) {
      notif.archived = true;
      notif.read = true;
      this.saveHistory();
      this.emit();
    }
  }

  archiveAll(): void {
    let changed = false;
    for (const n of this.history) {
      if (!n.archived) {
        n.archived = true;
        n.read = true;
        changed = true;
      }
    }
    if (changed) {
      this.saveHistory();
      this.emit();
    }
  }

  clearHistory(): void {
    this.history = [];
    this.saveHistory();
    this.emit();
  }

  // ─── Preferences ───────────────────────────────────────────────────────────

  getPreferences(): NotificationPreferences {
    return { ...this.preferences };
  }

  updatePreferences(patch: Partial<NotificationPreferences>): void {
    this.preferences = {
      ...this.preferences,
      ...patch,
      channels: { ...this.preferences.channels, ...(patch.channels ?? {}) },
      events: { ...this.preferences.events, ...(patch.events ?? {}) },
      webhookHeaders: {
        ...this.preferences.webhookHeaders,
        ...(patch.webhookHeaders ?? {}),
      },
    };
    this.savePreferences();
    this.emit();
  }

  setEventEnabled(eventType: NotificationEventType, enabled: boolean): void {
    this.preferences.events[eventType] = enabled;
    this.savePreferences();
  }

  setChannelEnabled(channel: NotificationChannel, enabled: boolean): void {
    this.preferences.channels[channel] = enabled;
    this.savePreferences();
  }

  // ─── Email delivery ────────────────────────────────────────────────────────

  /**
   * Email is delivered by posting a structured payload to an email gateway webhook.
   * This keeps the frontend stateless — a backend or serverless function receives
   * the payload and dispatches the actual email.
   *
   * The endpoint is configured via VITE_EMAIL_WEBHOOK_URL env variable.
   */
  private async deliverEmail(notification: AppNotification): Promise<void> {
    const endpoint = import.meta.env.VITE_EMAIL_WEBHOOK_URL;
    if (!endpoint) return;

    const payload = {
      to: this.preferences.emailAddress,
      subject: `[Atomic IP] ${notification.title}`,
      body: notification.message,
      event: notification.eventType,
      severity: notification.severity,
      timestamp: notification.timestamp,
      meta: notification.meta,
    };

    const response = await fetch(endpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      throw new Error(`Email gateway responded with ${response.status}`);
    }
  }

  // ─── Webhook delivery ──────────────────────────────────────────────────────

  /**
   * POST a structured webhook payload to the user-configured URL.
   * Standard headers plus any user-defined custom headers (e.g. Authorization).
   */
  private async deliverWebhook(notification: AppNotification): Promise<void> {
    const url = this.preferences.webhookUrl;
    if (!url) return;

    const payload: WebhookPayload = {
      event: notification.eventType,
      severity: notification.severity,
      title: notification.title,
      message: notification.message,
      timestamp: notification.timestamp,
      walletAddress: this.walletAddress ?? "",
      meta: notification.meta,
    };

    const headers: Record<string, string> = {
      "Content-Type": "application/json",
      "X-Atomic-IP-Event": notification.eventType,
      "X-Atomic-IP-Severity": notification.severity,
      ...this.preferences.webhookHeaders,
    };

    const response = await fetch(url, {
      method: "POST",
      headers,
      body: JSON.stringify(payload),
    });

    if (!response.ok) {
      throw new Error(`Webhook endpoint responded with ${response.status}`);
    }
  }

  // ─── Persistence ───────────────────────────────────────────────────────────

  private storageKey(suffix: string): string {
    const wallet = this.walletAddress ?? "anon";
    return `${suffix}_${wallet}`;
  }

  private saveHistory(): void {
    try {
      localStorage.setItem(
        this.storageKey(STORAGE_KEY_HISTORY),
        JSON.stringify(this.history)
      );
    } catch {
      // localStorage quota exceeded — prune old entries and retry once
      this.history = this.history.slice(0, Math.floor(MAX_HISTORY / 2));
      try {
        localStorage.setItem(
          this.storageKey(STORAGE_KEY_HISTORY),
          JSON.stringify(this.history)
        );
      } catch {
        // silent fail
      }
    }
  }

  private savePreferences(): void {
    try {
      localStorage.setItem(
        this.storageKey(STORAGE_KEY_PREFS),
        JSON.stringify(this.preferences)
      );
    } catch {
      // silent fail
    }
  }

  private loadFromStorage(): void {
    // Load history
    try {
      const raw = localStorage.getItem(this.storageKey(STORAGE_KEY_HISTORY));
      this.history = raw ? (JSON.parse(raw) as AppNotification[]) : [];
    } catch {
      this.history = [];
    }

    // Load preferences
    try {
      const raw = localStorage.getItem(this.storageKey(STORAGE_KEY_PREFS));
      if (raw) {
        const saved = JSON.parse(raw) as Partial<NotificationPreferences>;
        this.preferences = {
          ...DEFAULT_PREFERENCES,
          ...saved,
          channels: { ...DEFAULT_PREFERENCES.channels, ...(saved.channels ?? {}) },
          events: { ...DEFAULT_PREFERENCES.events, ...(saved.events ?? {}) },
        };
      } else {
        this.preferences = { ...DEFAULT_PREFERENCES };
      }
    } catch {
      this.preferences = { ...DEFAULT_PREFERENCES };
    }
  }
}

// ─── Singleton ─────────────────────────────────────────────────────────────────

export const notificationService = new NotificationService();

// ─── Convenience factory helpers ───────────────────────────────────────────────

/**
 * Predefined notification templates for each marketplace event.
 * Each factory returns the args to pass to notificationService.send().
 */
export const NotificationTemplates = {
  swapInitiated: (swapId: number, listingId: number, amountUsdc: number) => ({
    eventType: "swap_initiated" as NotificationEventType,
    title: "Swap Initiated",
    message: `A buyer has initiated Swap #${swapId} for Listing #${listingId} (${amountUsdc.toFixed(2)} USDC). Review and confirm to release the IP.`,
    severity: "info" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId, listingId, amountUsdc } },
  }),

  swapConfirmed: (swapId: number, listingId: number, amountUsdc: number) => ({
    eventType: "swap_confirmed" as NotificationEventType,
    title: "Swap Confirmed",
    message: `Swap #${swapId} for Listing #${listingId} was confirmed. ${amountUsdc.toFixed(2)} USDC has been released.`,
    severity: "success" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId, listingId, amountUsdc } },
  }),

  swapCancelled: (swapId: number) => ({
    eventType: "swap_cancelled" as NotificationEventType,
    title: "Swap Cancelled",
    message: `Swap #${swapId} has been cancelled and your USDC refunded.`,
    severity: "warning" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId } },
  }),

  swapExpired: (swapId: number) => ({
    eventType: "swap_expired" as NotificationEventType,
    title: "Swap Expired",
    message: `Swap #${swapId} has expired. You can now cancel it to reclaim your USDC.`,
    severity: "warning" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId } },
  }),

  swapDisputed: (swapId: number) => ({
    eventType: "swap_disputed" as NotificationEventType,
    title: "Swap Disputed",
    message: `Swap #${swapId} has been flagged as disputed. Please review the details.`,
    severity: "error" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId } },
  }),

  paymentConfirmed: (swapId: number, amountUsdc: number) => ({
    eventType: "payment_confirmed" as NotificationEventType,
    title: "Payment Confirmed",
    message: `Payment of ${amountUsdc.toFixed(2)} USDC for Swap #${swapId} has been confirmed on-chain.`,
    severity: "success" as NotificationSeverity,
    opts: { href: `/swap/${swapId}`, meta: { swapId, amountUsdc } },
  }),

  listingCreated: (listingId: number) => ({
    eventType: "listing_created" as NotificationEventType,
    title: "Listing Registered",
    message: `Your IP has been registered as Listing #${listingId} on the marketplace.`,
    severity: "success" as NotificationSeverity,
    opts: { meta: { listingId } },
  }),

  listingUpdated: (listingId: number, field: string) => ({
    eventType: "listing_updated" as NotificationEventType,
    title: "Listing Updated",
    message: `Listing #${listingId} has been updated: ${field} changed.`,
    severity: "info" as NotificationSeverity,
    opts: { meta: { listingId, field } },
  }),

  marketplaceAnnouncement: (text: string) => ({
    eventType: "marketplace_announcement" as NotificationEventType,
    title: "Marketplace Update",
    message: text,
    severity: "info" as NotificationSeverity,
    opts: {},
  }),
} as const;
