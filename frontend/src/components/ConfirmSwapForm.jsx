import React, { useState } from "react";
import { confirmSwap } from "../lib/contractClient";
import "./ConfirmSwapForm.css";

const USDC_DECIMALS = 7;

/**
 * ConfirmSwapForm
 *
 * Allows a seller to confirm a pending swap by submitting the decryption key,
 * which atomically releases USDC to the seller.
 *
 * Props:
 *   swap      - { id, listing_id, usdc_amount, status, buyer }
 *   wallet    - connected wallet { address, signTransaction }
 *   onSuccess - callback fired after successful confirmation
 */
export function ConfirmSwapForm({ swap, wallet, onSuccess }) {
  const [decryptionKey, setDecryptionKey] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  // Only sellers with a pending swap should see this
  if (swap.status !== "Pending") return null;

  const handleSubmit = async (e) => {
    e.preventDefault();
    setError(null);

    if (!decryptionKey.trim()) {
      setError("Decryption key cannot be empty.");
      return;
    }

    setLoading(true);
    try {
      await confirmSwap(swap.id, decryptionKey.trim(), wallet);
      setDecryptionKey("");
      onSuccess();
    } catch (err) {
      setError(err.message || "Failed to confirm swap. Please try again.");
    } finally {
      setLoading(false);
    }
  };

  return (
    <form className="confirm-swap-form" onSubmit={handleSubmit} noValidate>
      <div className="confirm-swap-form__meta">
        <span>Swap #{swap.id}</span>
        <span>{(swap.usdc_amount / Math.pow(10, USDC_DECIMALS)).toFixed(2)} USDC</span>
      </div>

      <label className="confirm-swap-form__label" htmlFor={`dk-${swap.id}`}>
        Decryption Key
      </label>
      <input
        id={`dk-${swap.id}`}
        className="confirm-swap-form__input"
        type="text"
        placeholder="0x..."
        value={decryptionKey}
        onChange={(e) => setDecryptionKey(e.target.value)}
        disabled={loading}
        autoComplete="off"
        spellCheck={false}
      />

      {error && (
        <p className="confirm-swap-form__error" role="alert">
          {error}
        </p>
      )}

      <button
        className="confirm-swap-form__btn"
        type="submit"
        disabled={loading || !decryptionKey.trim()}
        aria-busy={loading}
      >
        {loading ? (
          <span className="confirm-swap-spinner" aria-label="Confirming..." />
        ) : (
          "Confirm & Release USDC"
        )}
      </button>
    </form>
  );
}
