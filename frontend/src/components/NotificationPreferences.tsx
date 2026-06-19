import { useState } from "react";
import { useNotifications } from "../context/NotificationContext";
import type { NotificationEventType, NotificationChannel } from "../services/notificationService";
import "./NotificationPreferences.css";

const CHANNEL_LABELS: Record<NotificationChannel, string> = {
  in_app: "In-App Toasts",
  email: "Email",
  webhook: "Webhook",
};

const CHANNEL_DESCRIPTIONS: Record<NotificationChannel, string> = {
  in_app: "Show toast notifications inside the app",
  email: "Send an email via your configured gateway",
  webhook: "POST a JSON payload to your webhook URL",
};

const EVENT_LABELS: Record<NotificationEventType, string> = {
  swap_initiated: "Swap Initiated",
  swap_confirmed: "Swap Confirmed",
  swap_cancelled: "Swap Cancelled",
  swap_expired: "Swap Expired",
  swap_disputed: "Swap Disputed",
  payment_confirmed: "Payment Confirmed",
  listing_created: "Listing Created",
  listing_updated: "Listing Updated",
  marketplace_announcement: "Marketplace Announcements",
};

const EVENT_DESCRIPTIONS: Record<NotificationEventType, string> = {
  swap_initiated: "When a buyer initiates a swap on your listing",
  swap_confirmed: "When a seller confirms your swap",
  swap_cancelled: "When a swap is cancelled by either party",
  swap_expired: "When a pending swap passes its expiry time",
  swap_disputed: "When a swap is flagged as disputed",
  payment_confirmed: "When USDC payment is released on-chain",
  listing_created: "When your IP is successfully registered",
  listing_updated: "When your listing details are changed",
  marketplace_announcement: "Platform-wide updates and announcements",
};

/**
 * NotificationPreferences
 *
 * Settings panel rendered in the NotificationHistory "Settings" tab.
 * Lets users toggle channels, configure email/webhook endpoints,
 * and opt in/out of specific event types.
 */
export function NotificationPreferences() {
  const { preferences, setChannelEnabled, setEventEnabled, updatePreferences } =
    useNotifications();

  const [emailDraft, setEmailDraft] = useState(preferences.emailAddress ?? "");
  const [webhookUrlDraft, setWebhookUrlDraft] = useState(preferences.webhookUrl ?? "");
  const [webhookHeadersDraft, setWebhookHeadersDraft] = useState(
    Object.entries(preferences.webhookHeaders ?? {})
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n")
  );
  const [saved, setSaved] = useState(false);

  const handleSaveEndpoints = () => {
    // Parse header lines "Key: Value"
    const headers: Record<string, string> = {};
    webhookHeadersDraft
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean)
      .forEach((line) => {
        const idx = line.indexOf(":");
        if (idx > 0) {
          headers[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
        }
      });

    updatePreferences({
      emailAddress: emailDraft.trim(),
      webhookUrl: webhookUrlDraft.trim(),
      webhookHeaders: headers,
    });

    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const allEvents = Object.keys(EVENT_LABELS) as NotificationEventType[];

  return (
    <div className="np">
      {/* ── Channels ── */}
      <section className="np__section">
        <h4 className="np__section-title">Channels</h4>
        {(Object.keys(CHANNEL_LABELS) as NotificationChannel[]).map((ch) => (
          <label key={ch} className="np__toggle-row">
            <div className="np__toggle-info">
              <span className="np__toggle-label">{CHANNEL_LABELS[ch]}</span>
              <span className="np__toggle-desc">{CHANNEL_DESCRIPTIONS[ch]}</span>
            </div>
            <button
              className={`np__toggle ${preferences.channels[ch] ? "np__toggle--on" : ""}`}
              role="switch"
              aria-checked={preferences.channels[ch]}
              onClick={() => setChannelEnabled(ch, !preferences.channels[ch])}
              aria-label={`${CHANNEL_LABELS[ch]} ${preferences.channels[ch] ? "enabled" : "disabled"}`}
            >
              <span className="np__toggle-thumb" />
            </button>
          </label>
        ))}
      </section>

      {/* ── Email endpoint ── */}
      {preferences.channels.email && (
        <section className="np__section">
          <h4 className="np__section-title">Email Address</h4>
          <input
            className="np__input"
            type="email"
            placeholder="you@example.com"
            value={emailDraft}
            onChange={(e) => setEmailDraft(e.target.value)}
            aria-label="Email address for notifications"
          />
          <p className="np__hint">
            Emails are sent via <code>VITE_EMAIL_WEBHOOK_URL</code> gateway.
          </p>
        </section>
      )}

      {/* ── Webhook endpoint ── */}
      {preferences.channels.webhook && (
        <section className="np__section">
          <h4 className="np__section-title">Webhook URL</h4>
          <input
            className="np__input np__input--mono"
            type="url"
            placeholder="https://your-server.example.com/webhook"
            value={webhookUrlDraft}
            onChange={(e) => setWebhookUrlDraft(e.target.value)}
            aria-label="Webhook URL"
          />
          <h4 className="np__section-title np__section-title--sub">
            Custom Headers <span className="np__hint-inline">(one per line, Key: Value)</span>
          </h4>
          <textarea
            className="np__input np__input--mono np__textarea"
            placeholder={"Authorization: Bearer token123\nX-My-Header: value"}
            value={webhookHeadersDraft}
            onChange={(e) => setWebhookHeadersDraft(e.target.value)}
            rows={3}
            aria-label="Custom webhook headers"
          />
        </section>
      )}

      {/* Save endpoints button */}
      {(preferences.channels.email || preferences.channels.webhook) && (
        <div className="np__save-row">
          <button
            className={`np__save-btn ${saved ? "np__save-btn--saved" : ""}`}
            onClick={handleSaveEndpoints}
          >
            {saved ? "✓ Saved" : "Save Endpoints"}
          </button>
        </div>
      )}

      {/* ── Event types ── */}
      <section className="np__section">
        <h4 className="np__section-title">Event Types</h4>
        <p className="np__hint">Choose which events you want to be notified about.</p>
        {allEvents.map((evt) => {
          // If not explicitly set, default is enabled (true)
          const enabled = preferences.events[evt] !== false;
          return (
            <label key={evt} className="np__toggle-row np__toggle-row--compact">
              <div className="np__toggle-info">
                <span className="np__toggle-label">{EVENT_LABELS[evt]}</span>
                <span className="np__toggle-desc">{EVENT_DESCRIPTIONS[evt]}</span>
              </div>
              <button
                className={`np__toggle np__toggle--sm ${enabled ? "np__toggle--on" : ""}`}
                role="switch"
                aria-checked={enabled}
                onClick={() => setEventEnabled(evt, !enabled)}
                aria-label={`${EVENT_LABELS[evt]} notifications ${enabled ? "enabled" : "disabled"}`}
              >
                <span className="np__toggle-thumb" />
              </button>
            </label>
          );
        })}
      </section>

      <p className="np__privacy-note">
        Your preferences are stored locally. Opt-outs are respected immediately.
        Email and webhook payloads never include private keys or wallet secrets.
      </p>
    </div>
  );
}
