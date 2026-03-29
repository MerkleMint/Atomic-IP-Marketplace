import { useState } from "react";
import { setMerkleRoot } from "../lib/contractClient";
import "./SetMerkleRootForm.css";

interface Props {
  listingId: number;
  wallet: {
    address: string;
    signTransaction: (xdr: string) => Promise<string>;
  };
}

const HEX_64_RE = /^[0-9a-fA-F]{64}$/;

export function SetMerkleRootForm({ listingId, wallet }: Props) {
  const [root, setRoot] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  const rootError =
    root.length > 0 && !HEX_64_RE.test(root)
      ? "Root must be exactly 64 hex characters (32 bytes)."
      : null;

  const canSubmit = root.length > 0 && !rootError && !loading;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    setError(null);
    setSuccess(false);
    setLoading(true);
    try {
      await setMerkleRoot(listingId, root, wallet);
      setSuccess(true);
      setRoot("");
    } catch (err: any) {
      setError(err?.message ?? "Failed to set Merkle root.");
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="smrf">
      <form className="smrf__form" onSubmit={handleSubmit} noValidate>
        <p className="smrf__desc">
          Set the Merkle root for listing #{listingId}. The root must be a
          64-character hex string (32 bytes).
        </p>

        <label className="smrf__label" htmlFor={`smrf-root-${listingId}`}>
          Merkle Root (64-char hex)
        </label>
        <input
          id={`smrf-root-${listingId}`}
          className="smrf__input smrf__input--mono"
          type="text"
          placeholder="e.g. a3f1…c9d2"
          value={root}
          onChange={(e) => {
            setRoot(e.target.value.trim());
            setSuccess(false);
            setError(null);
          }}
          disabled={loading}
          aria-describedby={
            rootError
              ? `smrf-root-err-${listingId}`
              : success
              ? `smrf-success-${listingId}`
              : undefined
          }
          aria-invalid={!!rootError}
          maxLength={64}
          spellCheck={false}
          autoComplete="off"
        />

        {rootError && (
          <p
            id={`smrf-root-err-${listingId}`}
            className="smrf__error"
            role="alert"
          >
            {rootError}
          </p>
        )}

        {error && (
          <p className="smrf__error" role="alert">
            {error}
          </p>
        )}

        {success && (
          <p
            id={`smrf-success-${listingId}`}
            className="smrf__success"
            role="status"
          >
            Merkle root updated successfully.
          </p>
        )}

        <button
          type="submit"
          className="smrf__btn"
          disabled={!canSubmit}
          aria-busy={loading}
        >
          {loading ? "Submitting…" : "Set Merkle Root"}
        </button>
      </form>
    </div>
  );
}
