import React, { useState, useCallback } from "react";
import { getDecryptionKey, getListing } from "../lib/contractClient";
import { fetchFromIpfs, decryptAesGcm } from "../lib/ipfs";
import "./DecryptionKeyPanel.css";

interface Props {
  swapId: number;
  listingId: number;
  /** Key already decoded from the Swap struct (populated after confirm_swap) */
  cachedKey: string | null;
}

export function DecryptionKeyPanel({ swapId, listingId, cachedKey }: Props) {
  const [key, setKey] = useState<string | null>(cachedKey);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // IPFS decrypt state
  const [ipfsLoading, setIpfsLoading] = useState(false);
  const [ipfsError, setIpfsError] = useState<string | null>(null);
  const [ipfsContent, setIpfsContent] = useState<string | null>(null);

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

  const fetchAndDecrypt = useCallback(async () => {
    if (!key) return;
    setIpfsLoading(true);
    setIpfsError(null);
    setIpfsContent(null);
    try {
      const listing = await getListing(listingId);
      if (!listing?.ipfs_hash) throw new Error("Listing IPFS hash not found.");
      const ciphertext = await fetchFromIpfs(listing.ipfs_hash);
      const plaintext = await decryptAesGcm(ciphertext, key);
      setIpfsContent(plaintext);
    } catch (err) {
      setIpfsError(err instanceof Error ? err.message : "Failed to fetch or decrypt content.");
    } finally {
      setIpfsLoading(false);
    }
  }, [key, listingId]);

  return (
    <div className="dkp" role="region" aria-label="Decryption Key">
      <p className="dkp__title">Decryption Key</p>

      {!key && !loading && (
        <button className="dkp__reveal-btn" onClick={fetchKey} disabled={loading}>
          Reveal Key
        </button>
      )}

      {loading && <span className="dkp__spinner" aria-label="Loading decryption key" />}

      {error && <p className="dkp__error" role="alert">{error}</p>}

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

          <div className="dkp__ipfs-section">
            <button
              className="dkp__ipfs-btn"
              onClick={fetchAndDecrypt}
              disabled={ipfsLoading}
              aria-busy={ipfsLoading}
            >
              {ipfsLoading ? "Decrypting…" : "Fetch & Decrypt IP Asset"}
            </button>

            {ipfsLoading && <span className="dkp__spinner" aria-label="Decrypting content" />}

            {ipfsError && (
              <p className="dkp__error" role="alert">{ipfsError}</p>
            )}

            {ipfsContent && (
              <div className="dkp__ipfs-content">
                <p className="dkp__ipfs-label">Decrypted Content</p>
                <pre className="dkp__ipfs-pre">{ipfsContent}</pre>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
