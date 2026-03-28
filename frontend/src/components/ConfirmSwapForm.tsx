import React, { useState } from "react";
import { confirmSwap, getUsdcBalance } from "../lib/contractClient";
import type { ProofNode } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import "./ConfirmSwapForm.css";

const USDC_DECIMALS = 7;

interface Props {
  swap: Swap;
  wallet: Wallet;
  onSuccess: () => void;
}

function parseProofPath(raw: string): ProofNode[] {
  const parsed = JSON.parse(raw);
  if (!Array.isArray(parsed)) {
    throw new Error("Proof path must be a JSON array.");
  }
  return parsed.map((node: any, i: number) => {
    if (!node.sibling || typeof node.sibling !== "string") {
      throw new Error(`ProofNode[${i}].sibling must be a hex string.`);
    }
    if (typeof node.is_left !== "boolean") {
      throw new Error(`ProofNode[${i}].is_left must be a boolean.`);
    }
    const hex = node.sibling.replace(/^0x/, "");
    if (hex.length !== 64) {
      throw new Error(`ProofNode[${i}].sibling must be 64 hex chars (32 bytes), got ${hex.length}.`);
    }
    return { sibling: hex, is_left: node.is_left };
  });
}

export function ConfirmSwapForm({ swap, wallet, onSuccess }: Props) {
  const [decryptionKey, setDecryptionKey] = useState("");
  const [proofPath, setProofPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [newBalance, setNewBalance] = useState<number | null>(null);

  if (swap.status !== "Pending") return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setNewBalance(null);
    if (!decryptionKey.trim()) { setError("Decryption key cannot be empty."); return; }
    if (!proofPath.trim()) { setError("Proof path cannot be empty."); return; }
    let parsedPath: ProofNode[];
    try {
      parsedPath = parseProofPath(proofPath.trim());
    } catch (err) {
      setError(err instanceof Error ? `Invalid proof path: ${err.message}` : "Invalid proof path.");
      return;
    }
    setLoading(true);
    try {
      await confirmSwap(swap.id, decryptionKey.trim(), parsedPath, wallet);
      setDecryptionKey("");
      setProofPath("");
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
      <label className="confirm-swap-form__label" htmlFor={`pp-${swap.id}`}>Proof Path (JSON)</label>
      <textarea
        id={`pp-${swap.id}`}
        className="confirm-swap-form__input confirm-swap-form__textarea"
        placeholder='[{"sibling": "0x...64 hex chars...", "is_left": true}, ...]'
        value={proofPath}
        onChange={(e) => setProofPath(e.target.value)}
        disabled={loading}
        autoComplete="off"
        spellCheck={false}
        rows={3}
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
        disabled={loading || !decryptionKey.trim() || !proofPath.trim()}
        aria-busy={loading}
      >
        {loading && <span className="confirm-swap-spinner" aria-hidden="true" />}
        {loading ? "Confirming…" : "Confirm & Release USDC"}
      </button>
    </form>
  );
}
