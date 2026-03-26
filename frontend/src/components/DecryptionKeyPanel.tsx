import React, { useState, useCallback } from "react";
import { getDecryptionKey } from "../lib/contractClient";
import "./DecryptionKeyPanel.css";

interface Props {
  swapId: number;
  /** Key already decoded from the Swap struct (populated after confirm_swap) */
  cachedKey: string | null;
}

export function DecryptionKeyPanel({ swapId, cachedKey }: Props) {
  // Prefer the key already present in the swap object; only fetch on demand if absent
  const [key, setKey] = useState<string | null>(cachedKey);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const fetchKey = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await getDecryptionKey(swapId);
      if (!result) {
        setError("Decryption key not found on-chain for this swap.");
      } else {
        setKey(result);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to retrieve decryption key.");
    } finally {
      setLoading(false);
    }
  }, [swapId]);

  const copyToClipboard = useCallback(async () => {
    if (!key) return;
    try {
      await navigator.clipboard.writeText(key);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback for environments without clipboard API
      const el = document.createElement("textarea");
      el.value = key;
      el.style.position = "fixed";
      el.style.opacity = "0";
      document.body.appendChild(el);
      el.select();
      document.execCommand("copy");
      document.body.removeChild(el);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [key]);

  return (
    <div className="dkp" role="region" aria-label="Decryption Key">
      <p className="dkp__title">Decryption Key</p>

      {!key && !loading && (
        <button className="dkp__reveal-btn" onClick={fetchKey} disabled={loading}>
          Reveal Key
        </button>
      )}

      {loading && (
        <span className="dkp__spinner" aria-label="Loading decryption key" />
      )}

      {error && (
        <p className="dkp__error" role="alert">{error}</p>
      )}

      {key && (
        <>
          <div className="dkp__key-row">
            <code className="dkp__key-value" aria-label="Decryption key hex">{key}</code>
            <button
              className="dkp__copy-btn"
              onClick={copyToClipboard}
              aria-label="Copy decryption key"
            >
              {copied ? "Copied!" : "Copy"}
            </button>
          </div>

          <div className="dkp__warning" role="note">
            <span className="dkp__warning-icon" aria-hidden="true">⚠️</span>
            <span>
              Store this key securely. Anyone with this key can decrypt the purchased IP asset.
              Do not share it or store it in an insecure location.
            </span>
          </div>
        </>
      )}
    </div>
  );
}
