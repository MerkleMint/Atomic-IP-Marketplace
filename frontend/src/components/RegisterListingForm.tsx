import React, { useState } from "react";
import { registerIp } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import "./RegisterListingForm.css";

interface Props {
  wallet: Wallet;
  onSuccess?: () => void;
  onCancel?: () => void;
}

interface FormData {
  ipfsHash: string;
  merkleRoot: string;
  priceUsdc: string;
  royaltyBps: string;
  royaltyRecipient: string;
}

interface FormErrors {
  ipfsHash?: string;
  merkleRoot?: string;
  priceUsdc?: string;
  royaltyBps?: string;
  royaltyRecipient?: string;
}

type SubmitStatus = "idle" | "submitting" | "success" | "error";

export function RegisterListingForm({ wallet, onSuccess, onCancel }: Props) {
  const [formData, setFormData] = useState<FormData>({
    ipfsHash: "",
    merkleRoot: "",
    priceUsdc: "",
    royaltyBps: "2500", // Default 25%
    royaltyRecipient: wallet.address ?? "",
  });

  const [errors, setErrors] = useState<FormErrors>({});
  const [status, setStatus] = useState<SubmitStatus>("idle");
  const [submitError, setSubmitError] = useState<string | null>(null);

  const validate = (): boolean => {
    const newErrors: FormErrors = {};

    // IPFS hash validation (non-empty, hex)
    if (!formData.ipfsHash.trim()) {
      newErrors.ipfsHash = "IPFS hash is required.";
    } else if (!/^[0-9a-fA-F]+$/.test(formData.ipfsHash.replace(/^0x/, ""))) {
      newErrors.ipfsHash = "IPFS hash must be a valid hex string.";
    }

    // Merkle root validation (non-empty, 64-char hex = 32 bytes)
    if (!formData.merkleRoot.trim()) {
      newErrors.merkleRoot = "Merkle root is required.";
    } else {
      const cleaned = formData.merkleRoot.replace(/^0x/, "");
      if (!/^[0-9a-fA-F]{64}$/.test(cleaned)) {
        newErrors.merkleRoot = "Merkle root must be a 64-character hex string (32 bytes).";
      }
    }

    // Price validation (> 0)
    const price = parseFloat(formData.priceUsdc);
    if (!formData.priceUsdc.trim()) {
      newErrors.priceUsdc = "Price is required.";
    } else if (isNaN(price) || price <= 0) {
      newErrors.priceUsdc = "Price must be greater than 0.";
    }

    // Royalty bps validation (0-10000)
    const royalty = parseInt(formData.royaltyBps, 10);
    if (!formData.royaltyBps.trim()) {
      newErrors.royaltyBps = "Royalty BPS is required.";
    } else if (isNaN(royalty) || royalty < 0 || royalty > 10000) {
      newErrors.royaltyBps = "Royalty must be between 0 and 10000 basis points.";
    }

    // Royalty recipient validation (Stellar address format G...)
    if (!formData.royaltyRecipient.trim()) {
      newErrors.royaltyRecipient = "Royalty recipient address is required.";
    } else if (!/^G[A-Z0-9]{55}$/.test(formData.royaltyRecipient)) {
      newErrors.royaltyRecipient = "Must be a valid Stellar address (G...)";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleChange = (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement>) => {
    setFormData((prev) => ({ ...prev, [field]: e.target.value }));
    setErrors((prev) => ({ ...prev, [field]: undefined }));
    setStatus("idle");
    setSubmitError(null);
  };

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!validate()) return;

    setStatus("submitting");
    setSubmitError(null);

    try {
      await registerIp(
        formData.ipfsHash,
        formData.merkleRoot,
        parseInt(formData.royaltyBps, 10),
        formData.royaltyRecipient,
        parseFloat(formData.priceUsdc),
        wallet
      );
      setStatus("success");
      if (onSuccess) {
        setTimeout(onSuccess, 1500);
      }
    } catch (err) {
      setStatus("error");
      setSubmitError(err instanceof Error ? err.message : "Transaction failed. Please try again.");
    }
  };

  return (
    <div className="rlf">
      <div className="rlf__header">
        <h3 className="rlf__title">Register New IP</h3>
        {onCancel && (
          <button
            type="button"
            className="rlf__close"
            onClick={onCancel}
            aria-label="Close form"
          >
            ✕
          </button>
        )}
      </div>

      {status === "success" ? (
        <div className="rlf__success">
          <span className="rlf__success-icon" aria-hidden="true">✓</span>
          <p>IP registered successfully!</p>
        </div>
      ) : (
        <form className="rlf__form" onSubmit={handleSubmit} noValidate>
          <p className="rlf__desc">
            Register a new intellectual property listing on the blockchain.
          </p>

          <div className="rlf__field">
            <label className="rlf__label" htmlFor="ipfs-hash">
              IPFS Hash <span className="rlf__required">*</span>
            </label>
            <input
              id="ipfs-hash"
              className={`rlf__input rlf__input--mono ${errors.ipfsHash ? "rlf__input--error" : ""}`}
              type="text"
              value={formData.ipfsHash}
              onChange={handleChange("ipfsHash")}
              placeholder="e.g. Qm... or 0x..."
              spellCheck={false}
              aria-describedby={errors.ipfsHash ? "ipfs-hash-error" : undefined}
            />
            {errors.ipfsHash && (
              <p id="ipfs-hash-error" className="rlf__error" role="alert">{errors.ipfsHash}</p>
            )}
          </div>

          <div className="rlf__field">
            <label className="rlf__label" htmlFor="merkle-root">
              Merkle Root <span className="rlf__required">*</span>
            </label>
            <input
              id="merkle-root"
              className={`rlf__input rlf__input--mono ${errors.merkleRoot ? "rlf__input--error" : ""}`}
              type="text"
              value={formData.merkleRoot}
              onChange={handleChange("merkleRoot")}
              placeholder="e.g. 0xa3f1...c9d2 (64 hex chars)"
              maxLength={66}
              spellCheck={false}
              aria-describedby={errors.merkleRoot ? "merkle-root-error" : undefined}
            />
            {errors.merkleRoot && (
              <p id="merkle-root-error" className="rlf__error" role="alert">{errors.merkleRoot}</p>
            )}
          </div>

          <div className="rlf__field">
            <label className="rlf__label" htmlFor="price-usdc">
              Price (USDC) <span className="rlf__required">*</span>
            </label>
            <input
              id="price-usdc"
              className={`rlf__input ${errors.priceUsdc ? "rlf__input--error" : ""}`}
              type="number"
              min="0.0000001"
              step="0.0000001"
              value={formData.priceUsdc}
              onChange={handleChange("priceUsdc")}
              placeholder="e.g. 10.00"
              aria-describedby={errors.priceUsdc ? "price-usdc-error" : undefined}
            />
            {errors.priceUsdc && (
              <p id="price-usdc-error" className="rlf__error" role="alert">{errors.priceUsdc}</p>
            )}
          </div>

          <div className="rlf__row">
            <div className="rlf__field">
              <label className="rlf__label" htmlFor="royalty-bps">
                Royalty (BPS) <span className="rlf__required">*</span>
              </label>
              <input
                id="royalty-bps"
                className={`rlf__input ${errors.royaltyBps ? "rlf__input--error" : ""}`}
                type="number"
                min="0"
                max="10000"
                value={formData.royaltyBps}
                onChange={handleChange("royaltyBps")}
                placeholder="e.g. 2500 = 25%"
                aria-describedby={errors.royaltyBps ? "royalty-bps-error" : undefined}
              />
              {errors.royaltyBps && (
                <p id="royalty-bps-error" className="rlf__error" role="alert">{errors.royaltyBps}</p>
              )}
              <span className="rlf__hint">10000 BPS = 100%</span>
            </div>

            <div className="rlf__field">
              <label className="rlf__label" htmlFor="royalty-recipient">
                Royalty Recipient <span className="rlf__required">*</span>
              </label>
              <input
                id="royalty-recipient"
                className={`rlf__input rlf__input--mono ${errors.royaltyRecipient ? "rlf__input--error" : ""}`}
                type="text"
                value={formData.royaltyRecipient}
                onChange={handleChange("royaltyRecipient")}
                placeholder="G..."
                spellCheck={false}
                aria-describedby={errors.royaltyRecipient ? "royalty-recipient-error" : undefined}
              />
              {errors.royaltyRecipient && (
                <p id="royalty-recipient-error" className="rlf__error" role="alert">{errors.royaltyRecipient}</p>
              )}
            </div>
          </div>

          {submitError && (
            <p className="rlf__submit-error" role="alert">{submitError}</p>
          )}

          <div className="rlf__actions">
            {onCancel && (
              <button
                type="button"
                className="rlf__btn rlf__btn--secondary"
                onClick={onCancel}
                disabled={status === "submitting"}
              >
                Cancel
              </button>
            )}
            <button
              type="submit"
              className="rlf__btn rlf__btn--primary"
              disabled={status === "submitting"}
              aria-busy={status === "submitting"}
            >
              {status === "submitting" ? (
                <>
                  <span className="rlf__spinner" aria-hidden="true" />
                  Registering…
                </>
              ) : (
                "Register IP"
              )}
            </button>
          </div>
        </form>
      )}
    </div>
  );
}
