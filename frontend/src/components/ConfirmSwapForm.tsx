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
  const [errors, setErrors] = useState<{ decryptionKey?: string; proofPath?: string }>({});
  const [loading, setLoading] = useState(false);
  const [newBalance, setNewBalance] = useState<number | null>(null);

  if (swap.status !== "Pending") return null;

  const validateField = (field: "decryptionKey" | "proofPath", value: string): string | undefined => {
    if (!value.trim()) return `${field === "decryptionKey" ? "Decryption key" : "Proof path"} cannot be empty.`;
    if (field === "proofPath") {
      try {
        parseProofPath(value.trim());
      } catch (err) {
        return err instanceof Error ? `Invalid proof path: ${err.message}` : "Invalid proof path.";
      }
    }
    return undefined;
  };

  const handleBlur = (field: "decryptionKey" | "proofPath") => (e: React.FocusEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const error = validateField(field, e.target.value);
    setErrors((prev) => ({ ...prev, [field]: error }));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const dkError = validateField("decryptionKey", decryptionKey);
    const ppError = validateField("proofPath", proofPath);
    setErrors({ decryptionKey: dkError, proofPath: ppError });
    if (dkError || ppError) return;

    setLoading(true);
    setNewBalance(null);
    try {
      await confirmSwap(swap.id, decryptionKey.trim(), parseProofPath(proofPath.trim()), wallet);
      setDecryptionKey("");
      setProofPath("");
      const balance = await getUsdcBalance(wallet.address).catch(() => null);
      setNewBalance(balance);
      onSuccess();
    } catch (err) {
      setErrors({ ...errors, proofPath: err instanceof Error ? err.message : "Failed to confirm swap." });
    } finally {
      setLoading(false);
    }
  };

  const isFormValid = !validateField("decryptionKey", decryptionKey) && !validateField("proofPath", proofPath);

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
        className={`confirm-swap-form__input ${errors.decryptionKey ? "confirm-swap-form__input--error" : ""}`}
        type="text"
        placeholder="0x..."
        value={decryptionKey}
        onChange={(e) => { setDecryptionKey(e.target.value); setErrors((prev) => ({ ...prev, decryptionKey: undefined })); }}
        onBlur={handleBlur("decryptionKey")}
        disabled={loading}
        autoComplete="off"
        spellCheck={false}
        aria-describedby={errors.decryptionKey ? `dk-${swap.id}-error` : undefined}
      />
      {errors.decryptionKey && <p id={`dk-${swap.id}-error`} className="confirm-swap-form__error" role="alert">{errors.decryptionKey}</p>}

      <label className="confirm-swap-form__label" htmlFor={`pp-${swap.id}`}>Proof Path (JSON)</label>
      <textarea
        id={`pp-${swap.id}`}
        className={`confirm-swap-form__input confirm-swap-form__textarea ${errors.proofPath ? "confirm-swap-form__input--error" : ""}`}
        placeholder='[{"sibling": "0x...64 hex chars...", "is_left": true}, ...]'
        value={proofPath}
        onChange={(e) => { setProofPath(e.target.value); setErrors((prev) => ({ ...prev, proofPath: undefined })); }}
        onBlur={handleBlur("proofPath")}
        disabled={loading}
        autoComplete="off"
        spellCheck={false}
        rows={3}
        aria-describedby={errors.proofPath ? `pp-${swap.id}-error` : undefined}
      />
      {errors.proofPath && <p id={`pp-${swap.id}-error`} className="confirm-swap-form__error" role="alert">{errors.proofPath}</p>}

      {newBalance !== null && (
        <p className="confirm-swap-form__balance" role="status">
          USDC balance: {newBalance.toFixed(2)}
        </p>
      )}

      <button
        className="confirm-swap-form__btn"
        type="submit"
        disabled={loading || !isFormValid}
        aria-busy={loading}
      >
        {loading && <span className="confirm-swap-spinner" aria-hidden="true" />}
        {loading ? "Confirming…" : "Confirm & Release USDC"}
      </button>
    </form>
  );
}