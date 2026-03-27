import React, { useState } from "react";
import { confirmSwap, getUsdcBalance } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./ConfirmSwapForm.css";

const USDC_DECIMALS = 7;

interface Props {
  swap: Swap;
  wallet: Wallet;
  onSuccess: () => void;
}

export function ConfirmSwapForm({ swap, wallet, onSuccess }: Props) {
  const [decryptionKey, setDecryptionKey] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newBalance, setNewBalance] = useState<number | null>(null);

  if (swap.status !== "Pending") return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setNewBalance(null);
    if (!decryptionKey.trim()) { setError("Decryption key cannot be empty."); return; }
    setLoading(true);
    try {
      await confirmSwap(swap.id, decryptionKey.trim(), wallet);
      setDecryptionKey("");
      // Fetch updated balance after confirmation
      const balance = await getUsdcBalance(wallet.address).catch(() => null);
      setNewBalance(balance);
      onSuccess();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to confirm swap.");
    } finally {
      setLoading(false);
    }
  };

  const displayAmount = (swap.usdc_amount / Math.pow(10, USDC_DECIMALS)).toFixed(2);

  return (
    <form className="confirm-swap-form" onSubmit={handleSubmit} noValidate>
      <div className="confirm-swap-form__meta">
        <span>Swap #{swap.id}</span>
        <span>{displayAmount} USDC</span>
      </div>
      <label className="confirm-swap-form__label" htmlFor={`dk-${swap.id}`}>Decryption Key</label>
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
      {error && <p className="confirm-swap-form__error" role="alert">{error}</p>}
      {newBalance !== null && (
        <p className="confirm-swap-form__balance" role="status">
          USDC balance: {newBalance.toFixed(2)}
        </p>
      )}
      <button
        className="confirm-swap-form__btn"
        type="submit"
        disabled={loading || !decryptionKey.trim()}
        aria-busy={loading}
      >
        {loading && <span className="confirm-swap-spinner" aria-hidden="true" />}
        {loading ? "Confirming…" : "Confirm & Release USDC"}
      </button>
    </form>
  );
}
