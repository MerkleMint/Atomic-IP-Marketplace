import React, { useState } from "react";
import { setMerkleRoot, verifyPartialProof } from "../lib/contractClient";
import type { ProofNode } from "../lib/contractClient";
import type { Wallet } from "../lib/walletKit";
import "./SetMerkleRootForm.css";

interface Props {
  listingId: number;
  wallet: Wallet;
}

type Tab = "root" | "proof";

export function SetMerkleRootForm({ listingId, wallet }: Props) {
  const [tab, setTab] = useState<Tab>("root");

  // Set root state
  const [root, setRoot] = useState("");
  const [rootStatus, setRootStatus] = useState<"idle" | "submitting" | "success" | "error">("idle");
  const [rootError, setRootError] = useState<string | null>(null);

  // Proof builder state
  const [leafHex, setLeafHex] = useState("");
  const [proofNodes, setProofNodes] = useState<ProofNode[]>([{ sibling: "", is_left: false }]);
  const [verifyResult, setVerifyResult] = useState<boolean | null>(null);
  const [verifyError, setVerifyError] = useState<string | null>(null);
  const [verifying, setVerifying] = useState(false);

  const isValidHex32 = (v: string) => /^[0-9a-fA-F]{64}$/.test(v.replace(/^0x/, ""));

  const handleSetRoot = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!isValidHex32(root)) {
      setRootError("Root must be a 64-character hex string (32 bytes).");
      return;
    }
    setRootError(null);
    setRootStatus("submitting");
    try {
      await setMerkleRoot(listingId, root, wallet);
      setRootStatus("success");
    } catch (err) {
      setRootError(err instanceof Error ? err.message : "Transaction failed.");
      setRootStatus("error");
    }
  };

  const addNode = () =>
    setProofNodes((prev: ProofNode[]) => [...prev, { sibling: "", is_left: false }]);

  const removeNode = (i: number) =>
    setProofNodes((prev: ProofNode[]) => prev.filter((_: ProofNode, idx: number) => idx !== i));

  const updateNode = (i: number, field: keyof ProofNode, value: string | boolean) =>
    setProofNodes((prev: ProofNode[]) =>
      prev.map((n: ProofNode, idx: number) => (idx === i ? { ...n, [field]: value } : n))
    );

  const handleVerify = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setVerifyError(null);
    setVerifyResult(null);

    if (!leafHex.trim()) {
      setVerifyError("Leaf data is required.");
      return;
    }
    for (const node of proofNodes) {
      if (!isValidHex32(node.sibling)) {
        setVerifyError("Each sibling hash must be a 64-character hex string.");
        return;
      }
    }

    setVerifying(true);
    try {
      const result = await verifyPartialProof(listingId, leafHex, proofNodes);
      setVerifyResult(result);
    } catch (err) {
      setVerifyError(err instanceof Error ? err.message : "Verification failed.");
    } finally {
      setVerifying(false);
    }
  };

  return (
    <div className="smrf">
      <div className="smrf__tabs" role="tablist">
        <button
          role="tab"
          aria-selected={tab === "root"}
          className={`smrf__tab ${tab === "root" ? "smrf__tab--active" : ""}`}
          onClick={() => setTab("root")}
        >
          Set Merkle Root
        </button>
        <button
          role="tab"
          aria-selected={tab === "proof"}
          className={`smrf__tab ${tab === "proof" ? "smrf__tab--active" : ""}`}
          onClick={() => setTab("proof")}
        >
          Verify Proof
        </button>
      </div>

      {tab === "root" && (
        <form className="smrf__form" onSubmit={handleSetRoot}>
          <p className="smrf__desc">
            Submit a 32-byte Merkle root for listing #{listingId}.
          </p>
          <label className="smrf__label" htmlFor={`root-${listingId}`}>
            Merkle Root (64-char hex)
          </label>
          <input
            id={`root-${listingId}`}
            className="smrf__input smrf__input--mono"
            type="text"
            value={root}
            onChange={(e) => { setRoot(e.target.value); setRootStatus("idle"); }}
            placeholder="e.g. a3f1…c9d2 (64 hex chars)"
            maxLength={66}
            spellCheck={false}
            aria-describedby={rootError ? `root-err-${listingId}` : undefined}
          />
          {rootError && (
            <p id={`root-err-${listingId}`} className="smrf__error" role="alert">{rootError}</p>
          )}
          {rootStatus === "success" && (
            <p className="smrf__success" role="status">Merkle root set successfully.</p>
          )}
          <button
            className="smrf__btn"
            type="submit"
            disabled={rootStatus === "submitting"}
            aria-busy={rootStatus === "submitting"}
          >
            {rootStatus === "submitting" ? "Submitting…" : "Set Root"}
          </button>
        </form>
      )}

      {tab === "proof" && (
        <form className="smrf__form" onSubmit={handleVerify}>
          <p className="smrf__desc">
            Build a Merkle proof path and verify inclusion against the stored root for listing #{listingId}.
          </p>

          <label className="smrf__label" htmlFor={`leaf-${listingId}`}>
            Leaf Data (hex)
          </label>
          <input
            id={`leaf-${listingId}`}
            className="smrf__input smrf__input--mono"
            type="text"
            value={leafHex}
            onChange={(e) => setLeafHex(e.target.value)}
            placeholder="hex-encoded leaf bytes"
            spellCheck={false}
          />

          <div className="smrf__nodes-header">
            <span className="smrf__label">Proof Path</span>
            <button type="button" className="smrf__btn smrf__btn--sm" onClick={addNode}>
              + Add Node
            </button>
          </div>

          <ul className="smrf__nodes">
            {proofNodes.map((node: ProofNode, i: number) => (
              <li key={i} className="smrf__node">
                <span className="smrf__node-index">#{i}</span>
                <input
                  className="smrf__input smrf__input--mono smrf__input--sibling"
                  type="text"
                  value={node.sibling}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) => updateNode(i, "sibling", e.target.value)}
                  placeholder="sibling hash (64 hex chars)"
                  spellCheck={false}
                  aria-label={`Sibling hash for node ${i}`}
                />
                <label className="smrf__node-label">
                  <input
                    type="checkbox"
                    checked={node.is_left}
                    onChange={(e: React.ChangeEvent<HTMLInputElement>) => updateNode(i, "is_left", e.target.checked)}
                    aria-label={`Node ${i} sibling is on the left`}
                  />
                  Sibling is left
                </label>
                {proofNodes.length > 1 && (
                  <button
                    type="button"
                    className="smrf__btn smrf__btn--remove"
                    onClick={() => removeNode(i)}
                    aria-label={`Remove node ${i}`}
                  >
                    ✕
                  </button>
                )}
              </li>
            ))}
          </ul>

          {verifyError && (
            <p className="smrf__error" role="alert">{verifyError}</p>
          )}
          {verifyResult !== null && (
            <p
              className={`smrf__verify-result ${verifyResult ? "smrf__verify-result--valid" : "smrf__verify-result--invalid"}`}
              role="status"
            >
              {verifyResult ? "✓ Proof valid — leaf is included in the Merkle tree." : "✗ Proof invalid — leaf not found."}
            </p>
          )}

          <button
            className="smrf__btn"
            type="submit"
            disabled={verifying}
            aria-busy={verifying}
          >
            {verifying ? "Verifying…" : "Verify Proof"}
          </button>
        </form>
      )}
    </div>
  );
}
