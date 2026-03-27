import React, { useState } from "react";
import { registerIp } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit"; // Adjust if wallet type differs
import "./RegisterListingForm.css";

interface Props {
  wallet: Wallet;
  onSuccess?: (listingId: number) => void;
}

type Status = "idle" | "submitting" | "success" | "error";

export function RegisterListingForm({ wallet, onSuccess }: Props) {
  const [ipfsHash, setIpfsHash] = useState("");
  const [merkleRoot, setMerkleRoot] = useState("");
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);
  const [newListingId, setNewListingId] = useState<number | null>(null);

  const isValidHex32 = (v: string) => /^[0-9a-fA-F]{64}$/.test(v.replace(/^0x/, ""));
  const isValidIpfsCid = (v: string) => v.trim().length > 10; // Basic CID length check

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setStatus("submitting");

    if (!ipfsHash.trim()) {
      setError("IPFS hash is required.");
      setStatus("idle");
      return;
    }
    if (!isValidIpfsCid(ipfsHash)) {
      setError("IPFS hash appears invalid (should be a valid CID).");
      setStatus("idle");
      return;
    }
    if (merkleRoot && !isValidHex32(merkleRoot)) {
      setError("Merkle root must be a 64-character hex string (32 bytes).");
      setStatus("idle");
      return;
    }

    try {
      const listingId = await registerIp(ipfsHash.trim(), merkleRoot.trim() || null, wallet);
      setNewListingId(listingId);
      setStatus("success");
      onSuccess?.(listingId);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Registration failed.");
      setStatus("error");
    }
  };

  const resetForm = () => {
    setIpfsHash("");
    setMerkleRoot("");
    setStatus("idle");
    setError(null);
    setNewListingId(null);
  };

  return (
    <div className="rlf">
      <form className="rlf__form" onSubmit={handleSubmit}>
        <h3 className="rlf__title">Register New IP Listing</h3>
        <p className="rlf__desc">
          Submit your IPFS hash to register a new listing on the IP Registry contract.
        </p>

        <fieldset disabled={status === "submitting"} aria-busy={status === "submitting"}>
          <label className="rlf__label" htmlFor="ipfs-hash">
            IPFS Hash (CID)
          </label>
          <input
            id="ipfs-hash"
            className="rlf__input rlf__input--mono"
            type="text"
            value={ipfsHash}
            onChange={(e) => {
              setIpfsHash(e.target.value);
              if (status !== "submitting") setStatus("idle");
            }}
            placeholder="QmX... (IPFS CIDv0)"
            spellCheck={false}
            aria-describedby={error ? "ipfs-err" : undefined}
          />

          <label className="rlf__label" htmlFor="merkle-root">
            Merkle Root (optional, 64-char hex)
          </label>
          <input
            id="merkle-root"
            className="rlf__input rlf__input--mono"
            type="text"
            value={merkleRoot}
            onChange={(e) => {
              setMerkleRoot(e.target.value);
              if (status !== "submitting") setStatus("idle");
            }}
            placeholder="a1b2c3... (64 hex chars)"
            maxLength={66}
            spellCheck={false}
          />

          {error && (
            <p id="ipfs-err" className="rlf__error" role="alert">{error}</p>
          )}

          {status === "success" && newListingId && (
            <div className="rlf__success">
              <p className="rlf__success-msg" role="status">
                Listing registered successfully! ID: <strong>#{newListingId}</strong>
              </p>
              <button
                type="button"
                className="rlf__btn rlf__btn--secondary"
                onClick={resetForm}
              >
                Register Another
              </button>
            </div>
          )}

          <button
            className="rlf__btn rlf__btn--primary"
            type="submit"
            disabled={status === "submitting"}
          >
            {status === "submitting" ? "Registering..." : "Register Listing"}
          </button>
        </fieldset>
      </form>
    </div>
  );
}
