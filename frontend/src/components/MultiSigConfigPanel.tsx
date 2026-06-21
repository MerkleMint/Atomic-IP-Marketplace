/**
 * MultiSigConfigPanel
 *
 * Admin-only panel for configuring the multi-sig threshold and signer list.
 * Rendered inside AdminPanel under a new "Multi-Sig" tab.
 *
 * Supports:
 *   - Viewing current configuration
 *   - Setting threshold (USDC, human-readable)
 *   - Adding/removing signers (up to 3)
 *   - Choosing required approvals (2-of-2 or 2-of-3)
 *   - Enabling / disabling the multi-sig gate
 */
import { useState, useEffect } from "react";
import { useWallet } from "../context/WalletContext";
import { getMultiSigConfig, setMultiSigConfig } from "../lib/contractClient";
import type { MultiSigConfig } from "../lib/multiSigTypes";
import { rawToUsdc, DEFAULT_MULTISIG_THRESHOLD_USDC } from "../lib/multiSigTypes";
import "./MultiSigConfigPanel.css";

const MAX_SIGNERS = 3;

function shortAddr(addr: string): string {
  if (!addr || addr.length < 10) return addr;
  return `${addr.slice(0, 8)}…${addr.slice(-6)}`;
}

export function MultiSigConfigPanel() {
  const { wallet } = useWallet();

  // Current on-chain config
  const [current, setCurrent] = useState<MultiSigConfig | null>(null);
  const [loadingCurrent, setLoadingCurrent] = useState(true);

  // Form state
  const [threshold, setThreshold] = useState<string>(
    String(DEFAULT_MULTISIG_THRESHOLD_USDC)
  );
  const [signers, setSigners] = useState<string[]>(["", ""]);
  const [requiredApprovals, setRequiredApprovals] = useState(2);
  const [enabled, setEnabled] = useState(true);

  // UI state
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Load current config
  useEffect(() => {
    (async () => {
      setLoadingCurrent(true);
      try {
        const cfg = await getMultiSigConfig();
        setCurrent(cfg);
        if (cfg) {
          setThreshold(String(rawToUsdc(cfg.threshold)));
          setSigners(
            cfg.signers.length > 0
              ? [...cfg.signers, ...Array(Math.max(0, 2 - cfg.signers.length)).fill("")]
              : ["", ""]
          );
          setRequiredApprovals(cfg.required_approvals);
          setEnabled(cfg.enabled);
        }
      } catch {
        // non-fatal
      } finally {
        setLoadingCurrent(false);
      }
    })();
  }, []);

  function addSigner() {
    if (signers.length < MAX_SIGNERS) {
      setSigners([...signers, ""]);
    }
  }

  function removeSigner(i: number) {
    const next = signers.filter((_, idx) => idx !== i);
    setSigners(next.length > 0 ? next : [""]);
    if (requiredApprovals > next.filter(Boolean).length) {
      setRequiredApprovals(Math.max(1, next.filter(Boolean).length));
    }
  }

  function updateSigner(i: number, val: string) {
    const next = [...signers];
    next[i] = val.trim();
    setSigners(next);
  }

  function validateForm(): string | null {
    const thresholdNum = parseFloat(threshold);
    if (isNaN(thresholdNum) || thresholdNum <= 0) {
      return "Threshold must be a positive USDC amount.";
    }
    const validSigners = signers.filter(Boolean);
    if (validSigners.length < 2) {
      return "At least 2 signers are required.";
    }
    for (const s of validSigners) {
      if (!s.startsWith("G") || s.length !== 56) {
        return `Invalid Stellar address: ${shortAddr(s)}`;
      }
    }
    const unique = new Set(validSigners);
    if (unique.size !== validSigners.length) {
      return "Duplicate signer addresses are not allowed.";
    }
    if (requiredApprovals < 1 || requiredApprovals > validSigners.length) {
      return `Required approvals must be between 1 and ${validSigners.length}.`;
    }
    return null;
  }

  async function handleSave() {
    if (!wallet) return;
    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    setError(null);
    setSuccess(null);
    setSaving(true);

    try {
      const validSigners = signers.filter(Boolean);
      await setMultiSigConfig(
        parseFloat(threshold),
        validSigners,
        requiredApprovals,
        enabled,
        wallet
      );
      setSuccess(
        `Multi-sig configuration saved. Threshold: ${threshold} USDC, ` +
          `${requiredApprovals}-of-${validSigners.length} scheme, ` +
          `${enabled ? "enabled" : "disabled"}.`
      );
      // Refresh current config
      const cfg = await getMultiSigConfig();
      setCurrent(cfg);
    } catch (e: any) {
      setError(e?.message ?? "Failed to save configuration.");
    } finally {
      setSaving(false);
    }
  }

  const validSignersCount = signers.filter(Boolean).length;

  return (
    <div className="msc">
      <div>
        <h2 className="msc__title">Multi-Signature Configuration</h2>
        <p className="msc__desc">
          Swaps that exceed the USDC threshold require approval from multiple
          authorised signers before becoming active. Supports 2-of-2 and 2-of-3
          schemes with nonce-based replay-attack prevention.
        </p>
      </div>

      {/* Current config summary */}
      {!loadingCurrent && (
        <div className="msc__status-card" aria-label="Current multi-sig configuration">
          <div className="msc__status-item">
            <span className="msc__status-label">Status</span>
            <span
              className={`msc__status-value ${
                current?.enabled
                  ? "msc__status-value--enabled"
                  : "msc__status-value--disabled"
              }`}
            >
              {current?.enabled ? "● Enabled" : "○ Disabled"}
            </span>
          </div>
          <div className="msc__status-item">
            <span className="msc__status-label">Threshold</span>
            <span className="msc__status-value">
              {current
                ? `${rawToUsdc(current.threshold).toLocaleString()} USDC`
                : "—"}
            </span>
          </div>
          <div className="msc__status-item">
            <span className="msc__status-label">Scheme</span>
            <span className="msc__status-value">
              {current
                ? `${current.required_approvals}-of-${current.signers.length}`
                : "—"}
            </span>
          </div>
          <div className="msc__status-item">
            <span className="msc__status-label">Signers</span>
            <span className="msc__status-value">
              {current ? current.signers.length : "—"}
            </span>
          </div>
        </div>
      )}

      {/* Configuration form */}
      <div className="msc__form" aria-label="Multi-sig configuration form">
        <h3 className="msc__form-title">Update Configuration</h3>

        {/* Threshold */}
        <div className="msc__field">
          <label className="msc__label" htmlFor="msc-threshold">
            Threshold (USDC)
          </label>
          <input
            id="msc-threshold"
            className="msc__input"
            type="number"
            min="1"
            step="100"
            placeholder="10000"
            value={threshold}
            onChange={(e) => setThreshold(e.target.value)}
            disabled={saving}
          />
          <span className="msc__hint">
            Swaps at or above this amount require multi-sig approval.
          </span>
        </div>

        {/* Signers */}
        <div className="msc__field">
          <span className="msc__label">
            Authorised Signers (max {MAX_SIGNERS})
          </span>
          <div className="msc__signers">
            {signers.map((s, i) => (
              <div key={i} className="msc__signer-row">
                <input
                  className={`msc__input msc__signer-input ${
                    s && (!s.startsWith("G") || s.length !== 56)
                      ? "msc__input--error"
                      : ""
                  }`}
                  type="text"
                  placeholder={`Signer ${i + 1} — G...`}
                  value={s}
                  onChange={(e) => updateSigner(i, e.target.value)}
                  disabled={saving}
                  spellCheck={false}
                  autoComplete="off"
                  aria-label={`Signer ${i + 1} address`}
                />
                {signers.length > 2 && (
                  <button
                    type="button"
                    className="msc__signer-remove"
                    onClick={() => removeSigner(i)}
                    disabled={saving}
                    aria-label={`Remove signer ${i + 1}`}
                  >
                    ✕
                  </button>
                )}
              </div>
            ))}
          </div>
          {signers.length < MAX_SIGNERS && (
            <button
              type="button"
              className="msc__add-signer-btn"
              onClick={addSigner}
              disabled={saving}
            >
              + Add signer
            </button>
          )}
        </div>

        {/* Required approvals */}
        <div className="msc__field">
          <label className="msc__label" htmlFor="msc-required">
            Required Approvals
          </label>
          <select
            id="msc-required"
            className="msc__input"
            value={requiredApprovals}
            onChange={(e) => setRequiredApprovals(Number(e.target.value))}
            disabled={saving}
          >
            {Array.from(
              { length: Math.max(0, validSignersCount) },
              (_, i) => i + 1
            ).map((n) => (
              <option key={n} value={n}>
                {n}-of-{validSignersCount}
              </option>
            ))}
          </select>
          <span className="msc__hint">
            Minimum number of distinct approvals to unblock a high-value swap.
            Recommended: 2-of-2 or 2-of-3.
          </span>
        </div>

        {/* Enable / disable toggle */}
        <div className="msc__field">
          <div className="msc__toggle-row">
            <label className="msc__toggle" aria-label="Enable multi-sig">
              <input
                type="checkbox"
                checked={enabled}
                onChange={(e) => setEnabled(e.target.checked)}
                disabled={saving}
              />
              <span className="msc__toggle-slider" />
            </label>
            <span className="msc__toggle-label">
              {enabled ? "Multi-sig gate enabled" : "Multi-sig gate disabled"}
            </span>
          </div>
          <span className="msc__hint">
            When disabled, all swaps proceed normally regardless of amount.
          </span>
        </div>

        {/* Save / Disable actions */}
        <div className="msc__actions">
          <button
            type="button"
            className="msc__btn msc__btn--primary"
            onClick={handleSave}
            disabled={saving || !wallet}
            aria-busy={saving}
          >
            {saving ? (
              <>
                <span className="msc__spinner" aria-hidden="true" />
                Saving…
              </>
            ) : (
              "Save Configuration"
            )}
          </button>

          {current?.enabled && (
            <button
              type="button"
              className="msc__btn msc__btn--danger"
              onClick={() => {
                setEnabled(false);
                // auto-save the disable
                setTimeout(handleSave, 0);
              }}
              disabled={saving || !wallet}
            >
              Disable Multi-Sig
            </button>
          )}
        </div>

        {!wallet && (
          <p className="msc__hint">Connect your wallet to save changes.</p>
        )}

        {error && (
          <p className="msc__error" role="alert">
            {error}
          </p>
        )}
        {success && !error && (
          <p className="msc__success" role="status">
            {success}
          </p>
        )}
      </div>
    </div>
  );
}
