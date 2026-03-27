import React, { useState, useEffect, useRef } from "react";
import { approveUsdc, initiateSwap } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import "./InitiateSwapModal.css";

const USDC_CONTRACT_ID = import.meta.env.VITE_CONTRACT_USDC ?? "";
const ZK_VERIFIER_CONTRACT_ID = import.meta.env.VITE_CONTRACT_ZK_VERIFIER ?? "";
const USDC_DECIMALS = 7; // Stellar USDC uses 7 decimal places

export interface Listing {
  id: number;
  owner: string;
  ipfs_hash: string;
  price_usdc: number;
}

interface Props {
  listing: Listing;
  wallet: Wallet;
  onClose: () => void;
  onSuccess: (swapId: number) => void;
}

type Step = "input" | "approving" | "initiating" | "success";

export function InitiateSwapModal({ listing, wallet, onClose, onSuccess }: Props) {
  const [amount, setAmount] = useState(
    listing.price_usdc > 0
      ? String(listing.price_usdc / Math.pow(10, USDC_DECIMALS))
      : ""
  );
  const [step, setStep] = useState<Step>("input");
  const [error, setError] = useState<string | null>(null);
  const [swapId, setSwapId] = useState<number | null>(null);
  const backdropRef = useRef<HTMLDivElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  // Close on Escape
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") onClose(); };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Focus trap
  useEffect(() => { modalRef.current?.focus(); }, []);

  const usdcAmountRaw = (): bigint => {
    const parsed = parseFloat(amount);
    if (isNaN(parsed) || parsed <= 0) return 0n;
    return BigInt(Math.round(parsed * Math.pow(10, USDC_DECIMALS)));
  };

  const minAmount = listing.price_usdc > 0
    ? listing.price_usdc / Math.pow(10, USDC_DECIMALS)
    : 0;

  const isValidAmount = parseFloat(amount) > 0 && parseFloat(amount) >= minAmount;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!isValidAmount) {
      setError(minAmount > 0 ? `Minimum amount is ${minAmount} USDC.` : "Enter a valid USDC amount.");
      return;
    }

    const raw = usdcAmountRaw();

    try {
      // Step 1: Approve USDC
      setStep("approving");
      await approveUsdc(USDC_CONTRACT_ID, import.meta.env.VITE_CONTRACT_ATOMIC_SWAP, raw, wallet);

      // Step 2: Initiate swap
      setStep("initiating");
      const id = await initiateSwap(
        listing.id,
        listing.owner,
        USDC_CONTRACT_ID,
        raw,
        ZK_VERIFIER_CONTRACT_ID,
        wallet
      );

      setSwapId(id);
      setStep("success");
      onSuccess(id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Transaction failed.");
      setStep("input");
    }
  };

  const busy = step === "approving" || step === "initiating";

  return (
    <div
      className="ism-backdrop"
      ref={backdropRef}
      role="presentation"
      onClick={(e) => { if (e.target === backdropRef.current) onClose(); }}
    >
      <div
        className="ism"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ism-title"
        ref={modalRef}
        tabIndex={-1}
      >
        <div className="ism__header">
          <h2 className="ism__title" id="ism-title">Initiate Swap</h2>
          <button className="ism__close" onClick={onClose} aria-label="Close" disabled={busy}>×</button>
        </div>

        {/* Listing details */}
        <div className="ism__listing">
          <div className="ism__listing-row">
            <span className="ism__listing-label">Listing</span>
            <span className="ism__listing-value">#{listing.id}</span>
          </div>
          <div className="ism__listing-row">
            <span className="ism__listing-label">Seller</span>
            <span className="ism__listing-value ism__listing-value--mono">
              {listing.owner.slice(0, 6)}…{listing.owner.slice(-4)}
            </span>
          </div>
          <div className="ism__listing-row">
            <span className="ism__listing-label">IPFS Hash</span>
            <span className="ism__listing-value ism__listing-value--mono">
              {listing.ipfs_hash.slice(0, 20)}…
            </span>
          </div>
          {listing.price_usdc > 0 && (
            <div className="ism__listing-row">
              <span className="ism__listing-label">Min Price</span>
              <span className="ism__listing-value">{minAmount} USDC</span>
            </div>
          )}
        </div>

        {step === "success" ? (
          <div className="ism__success">
            <span className="ism__success-icon" aria-hidden="true">✅</span>
            <p>Swap initiated successfully!</p>
            <p className="ism__swap-id">Swap ID: <strong>#{swapId}</strong></p>
            <button className="ism__btn ism__btn--primary" onClick={onClose}>Close</button>
          </div>
        ) : (
          <form className="ism__form" onSubmit={handleSubmit} noValidate>
            <label className="ism__label" htmlFor="ism-amount">USDC Amount</label>
            <div className="ism__input-wrap">
              <input
                id="ism-amount"
                className="ism__input"
                type="number"
                min={minAmount > 0 ? minAmount : "0.0000001"}
                step="0.0000001"
                placeholder={minAmount > 0 ? `Min ${minAmount}` : "e.g. 10"}
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                disabled={busy}
                autoComplete="off"
              />
              <span className="ism__input-suffix">USDC</span>
            </div>

            {error && <p className="ism__error" role="alert">{error}</p>}

            <div className="ism__steps">
              <div className={`ism__step ${step === "approving" ? "ism__step--active" : ""} ${step === "initiating" ? "ism__step--done" : ""}`}>
                <span className="ism__step-num">1</span>
                <span>Approve USDC</span>
                {step === "approving" && <span className="ism__spinner" aria-hidden="true" />}
              </div>
              <div className={`ism__step ${step === "initiating" ? "ism__step--active" : ""}`}>
                <span className="ism__step-num">2</span>
                <span>Initiate Swap</span>
                {step === "initiating" && <span className="ism__spinner" aria-hidden="true" />}
              </div>
            </div>

            <button
              className="ism__btn ism__btn--primary"
              type="submit"
              disabled={busy || !isValidAmount}
              aria-busy={busy}
            >
              {step === "approving" && "Approving USDC…"}
              {step === "initiating" && "Initiating Swap…"}
              {step === "input" && "Approve & Initiate Swap"}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
