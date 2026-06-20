import React, {
  createContext,
  useContext,
  useEffect,
  useState,
  useCallback,
  useRef,
} from "react";
import {
  notificationService,
  type AppNotification,
  type NotificationPreferences,
  type NotificationEventType,
  type NotificationChannel,
  type NotificationSeverity,
} from "../services/notificationService";
import { useWallet } from "./WalletContext";

// ─── Context value ─────────────────────────────────────────────────────────────

interface NotificationContextValue {
  /** All non-archived notifications */
  notifications: AppNotification[];
  /** Count of unread, non-archived notifications */
  unreadCount: number;
  /** Current user preferences */
  preferences: NotificationPreferences;

  /** Send a notification programmatically */
  send: (
    eventType: NotificationEventType,
    title: string,
    message: string,
    severity?: NotificationSeverity,
    opts?: { href?: string; meta?: Record<string, unknown> }
  ) => Promise<AppNotification | null>;

  markRead: (id: string) => void;
  markAllRead: () => void;
  archive: (id: string) => void;
  archiveAll: () => void;
  clearHistory: () => void;

  /** Fetch history including archived entries */
  getFullHistory: () => AppNotification[];

  updatePreferences: (patch: Partial<NotificationPreferences>) => void;
  setChannelEnabled: (channel: NotificationChannel, enabled: boolean) => void;
  setEventEnabled: (event: NotificationEventType, enabled: boolean) => void;
}

const NotificationContext = createContext<NotificationContextValue | null>(null);

// ─── Provider ──────────────────────────────────────────────────────────────────

export function NotificationProvider({ children }: { children: React.ReactNode }) {
  const { wallet } = useWallet();

  const [notifications, setNotifications] = useState<AppNotification[]>([]);
  const [unreadCount, setUnreadCount] = useState(0);
  const [preferences, setPreferences] = useState<NotificationPreferences>(
    notificationService.getPreferences()
  );

  // Keep wallet address in sync with service so per-wallet storage keys are correct
  useEffect(() => {
    notificationService.setWalletAddress(wallet?.address ?? null);
    // Re-read state after wallet change
    setNotifications(notificationService.getHistory(false));
    setUnreadCount(notificationService.getUnreadCount());
    setPreferences(notificationService.getPreferences());
  }, [wallet?.address]);

  // Subscribe to service state changes
  useEffect(() => {
    const unsub = notificationService.subscribe(() => {
      setNotifications(notificationService.getHistory(false));
      setUnreadCount(notificationService.getUnreadCount());
      setPreferences(notificationService.getPreferences());
    });
    return unsub;
  }, []);

  const send = useCallback(
    (
      eventType: NotificationEventType,
      title: string,
      message: string,
      severity?: NotificationSeverity,
      opts?: { href?: string; meta?: Record<string, unknown> }
    ) => notificationService.send(eventType, title, message, severity, opts),
    []
  );

  const markRead = useCallback((id: string) => notificationService.markRead(id), []);
  const markAllRead = useCallback(() => notificationService.markAllRead(), []);
  const archive = useCallback((id: string) => notificationService.archive(id), []);
  const archiveAll = useCallback(() => notificationService.archiveAll(), []);
  const clearHistory = useCallback(() => notificationService.clearHistory(), []);
  const getFullHistory = useCallback(
    () => notificationService.getHistory(true),
    []
  );

  const updatePreferences = useCallback(
    (patch: Partial<NotificationPreferences>) => {
      notificationService.updatePreferences(patch);
      setPreferences(notificationService.getPreferences());
    },
    []
  );

  const setChannelEnabled = useCallback(
    (channel: NotificationChannel, enabled: boolean) => {
      notificationService.setChannelEnabled(channel, enabled);
      setPreferences(notificationService.getPreferences());
    },
    []
  );

  const setEventEnabled = useCallback(
    (event: NotificationEventType, enabled: boolean) => {
      notificationService.setEventEnabled(event, enabled);
      setPreferences(notificationService.getPreferences());
    },
    []
  );

  return (
    <NotificationContext.Provider
      value={{
        notifications,
        unreadCount,
        preferences,
        send,
        markRead,
        markAllRead,
        archive,
        archiveAll,
        clearHistory,
        getFullHistory,
        updatePreferences,
        setChannelEnabled,
        setEventEnabled,
      }}
    >
      {children}
    </NotificationContext.Provider>
  );
}

// ─── Hook ──────────────────────────────────────────────────────────────────────

export function useNotifications(): NotificationContextValue {
  const ctx = useContext(NotificationContext);
  if (!ctx) {
    throw new Error("useNotifications must be used inside <NotificationProvider>");
  }
  return ctx;
}

// ─── Swap event watcher ────────────────────────────────────────────────────────

/**
 * useSwapNotifications
 *
 * Watches a list of swaps and fires notifications whenever a swap transitions
 * to a new status. Call this inside any component that already polls swap data.
 *
 * @param swaps        - current list of swaps from useMySwaps
 * @param walletAddress - connected wallet address (to determine buyer/seller role)
 */
export function useSwapNotifications(
  swaps: Array<{
    id: number;
    listing_id: number;
    buyer: string;
    seller: string;
    status: string;
    usdc_amount: number;
    expires_at: number;
  }>,
  walletAddress: string | null
): void {
  const { send } = useNotifications();
  // Map of swapId → last known status so we only fire on transitions
  const prevStatusRef = useRef<Map<number, string>>(new Map());

  useEffect(() => {
    if (!walletAddress) return;

    const now = Math.floor(Date.now() / 1000);

    for (const swap of swaps) {
      const prev = prevStatusRef.current.get(swap.id);
      const curr = swap.status;
      const amountUsdc = swap.usdc_amount / Math.pow(10, 7); // USDC_DECIMALS

      // Only fire if status actually changed (or we're seeing it for the first time)
      if (prev === curr) continue;

      // Don't notify on initial load for settled swaps (avoids spam on reconnect)
      if (prev === undefined && curr !== "Pending") {
        prevStatusRef.current.set(swap.id, curr);
        continue;
      }

      prevStatusRef.current.set(swap.id, curr);

      const isBuyer = walletAddress === swap.buyer;
      const isSeller = walletAddress === swap.seller;

      switch (curr) {
        case "Pending": {
          if (prev === undefined && isSeller) {
            // New pending swap against seller's listing
            send(
              "swap_initiated",
              "New Swap Incoming",
              `Buyer has initiated Swap #${swap.id} for Listing #${swap.listing_id} (${amountUsdc.toFixed(2)} USDC). Confirm to release the IP.`,
              "info",
              { href: `/swap/${swap.id}`, meta: { swapId: swap.id, listingId: swap.listing_id } }
            );
          }
          break;
        }
        case "Completed": {
          if (isBuyer) {
            send(
              "swap_confirmed",
              "Swap Completed",
              `Swap #${swap.id} completed successfully. You now have access to the IP.`,
              "success",
              { href: `/swap/${swap.id}`, meta: { swapId: swap.id } }
            );
          }
          if (isSeller) {
            send(
              "payment_confirmed",
              "Payment Received",
              `Payment of ${amountUsdc.toFixed(2)} USDC for Swap #${swap.id} has been released to your wallet.`,
              "success",
              { href: `/swap/${swap.id}`, meta: { swapId: swap.id, amountUsdc } }
            );
          }
          break;
        }
        case "Cancelled": {
          if (isBuyer) {
            send(
              "swap_cancelled",
              "Swap Cancelled",
              `Swap #${swap.id} was cancelled. Your USDC has been refunded.`,
              "warning",
              { href: `/swap/${swap.id}`, meta: { swapId: swap.id } }
            );
          }
          if (isSeller) {
            send(
              "swap_cancelled",
              "Swap Cancelled by Buyer",
              `Buyer cancelled Swap #${swap.id} for Listing #${swap.listing_id}.`,
              "warning",
              { href: `/swap/${swap.id}`, meta: { swapId: swap.id } }
            );
          }
          break;
        }
        case "Disputed": {
          send(
            "swap_disputed",
            "Swap Disputed",
            `Swap #${swap.id} has been flagged as disputed. Please review.`,
            "error",
            { href: `/swap/${swap.id}`, meta: { swapId: swap.id } }
          );
          break;
        }
      }

      // Expiry warning: fire once when swap is Pending and past expires_at
      if (curr === "Pending" && isBuyer && now >= swap.expires_at && now < swap.expires_at + 120) {
        send(
          "swap_expired",
          "Swap Expired",
          `Swap #${swap.id} has expired. You can now cancel it to reclaim your USDC.`,
          "warning",
          { href: `/swap/${swap.id}`, meta: { swapId: swap.id } }
        );
      }
    }
  }, [swaps, walletAddress, send]);
}
