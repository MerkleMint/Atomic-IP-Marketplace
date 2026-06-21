/**
 * MultiSigApprovalPanel
 *
 * Displays the multi-sig approval status for a high-value swap and allows
 * configured signers to submit their on-chain approval.
 *
 * Rendered inside SwapCard when swap.status === "PendingMultiSig".
 */
import { useEffect, useState, useCallback } from "react";
import type { Wallet } from "../lib/walletKit";
import {
  getMultiSigConfig,
  getMultiSigApproval,
  approveMultiSigSwap,
} from "../lib/contractClient";
import type { MultiSigConfig, MultiSigApproval, SignerInfo } from "../lib/multiSigTypes";
import "./MultiSigApprovalPanel.css";

interface Props {
  swapId: number;
  usdcAmount: number;
  wallet: Wallet;
  onApproved: () => void;
}

function shortAddr(addr: string): string {
  if (!addr) return "";
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}

export function MultiSigApprovalPanel({
  swapId,
  usdcAmount,
  wallet,
  onApproved,
}: Props) {
  const [config, setConfig] = useState<MultiSigConfig | null>(null);
  const [approval, setApproval] = useState<MultiSigApproval | null>(null);
  const [loading, setLoading] = useState(true);
  const [approving, setApproving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [cfg, appr] = await Promise.all([
        getMultiSigConfig(),
        getMultiSigApproval(swapId),
      ]);
      setConfig(cfg);
      setApproval(appr);
    } catch (e) {
      // non-fatal — panel is supplementary information
    } finally {
      setLoading(false);
    }
  }, [swapId]);

  useEffect(() => {
    load();
  }, [load]);

  if (loading) {
    return (
      <div className="multisig-panel multisig-panel--pending" aria-busy="true">
        <span className="multisig-panel__spinner" style={{ borderColor: "#f59e0b", borderTopColor: "#92400e" }} />
        <span style={{ marginLeft: "0.5rem", fontSize: "0.8rem", color: "#92400e" }}>
          Loading multi-sig status…
        </span>
      </div>
    );
  }

  if (!config || !config.enabled) {
    return null; // multi-sig not configured — shouldn't happen for PendingMultiSig swaps
  }

  const approvedBy = approval?.approved_by ?? [];
  const approvedCount = approvedBy.length;
  const required = config.required_approvals;
  const thresholdMet = approvedCount >= required;

  // Build signer info list with approval status
  const signerInfos: SignerInfo[] = config.signers.map((addr) => ({
    address: addr,
    status: approvedBy.includes(addr) ? "approved" : "pending",
  }));

  const walletIsSigner = config.signers.includes(wallet.address);
  const walletAlreadyApproved = approvedBy.includes(wallet.address);
  const canApprove = walletIsSigner && !walletAlreadyApproved && !thresholdMet;

  const progressPct = Math.min(100, (approvedCount / required) * 100);

  async function handleApprove() {
    setError(null);
    setApproving(true);
    try {
      await approveMultiSigSwap(swapId, wallet);
      setSuccess("Approval submitted successfully.");
      await load();
      // Notify parent to refresh swap status — may now be Pending
      onApproved();
    } catch (e: any) {
      setError(e?.message ?? "Failed to submit approval.");
    } finally {
      setApproving(false);
    }
  }

  return (
    <div
      className={`multisig-panel ${thresholdMet ? "multisig-panel--approved" : "multisig-panel--pending"}`}
      aria-label="Multi-sig approval panel"
    >
      {/* Header */}
      <div className="multisig-panel__header">
        <h4 className="multisig-panel__title">
          <span aria-hidden="true">🔐</span>
          Multi-sig Required
        </h4>
        <span
          className={`multisig-panel__badge ${
            thresholdMet ? "multisig-panel__badge--met" : "multisig-panel__badge--waiting"
          }`}
          role="status"
          aria-live="polite"
        >
          {thresholdMet ? "✓ Approved" : `${approvedCount}/${required} signed`}
        </span>
      </div>

      {/* Threshold info */}
      <p className="multisig-panel__info">
        This swap exceeds the {(usdcAmount / 1e7).toLocaleString()} USDC threshold and requires{" "}
        <strong>{required} of {config.signers.length}</strong> authorised signers to approve
        before it can proceed.
      </p>

      {/* Progress bar */}
      <div className="multisig-panel__progress-wrap">
        <div className="multisig-panel__progress-label">
          {approvedCount} of {required} approvals received
        </div>
        <div className="multisig-panel__progress-bar-bg" role="progressbar"
          aria-valuenow={approvedCount}
          aria-valuemin={0}
          aria-valuemax={required}
        >
          <div
            className={`multisig-panel__progress-bar-fill ${thresholdMet ? "multisig-panel__progress-bar-fill--met" : ""}`}
            style={{ width: `${progressPct}%` }}
          />
        </div>
      </div>

      {/* Signer list */}
      <ul className="multisig-panel__signers" aria-label="Signer status">
        {signerInfos.map((s) => (
          <li key={s.address} className="multisig-panel__signer">
            <span
              className={`multisig-panel__signer-icon ${
                s.status === "approved"
                  ? "multisig-panel__signer-icon--approved"
                  : "multisig-panel__signer-icon--pending"
              }`}
              aria-hidden="true"
            >
              {s.status === "approved" ? "✓" : "○"}
            </span>
            <span className="multisig-panel__signer-addr" title={s.address}>
              {shortAddr(s.address)}
            </span>
            {s.address === wallet.address && (
              <span className="multisig-panel__signer-you">You</span>
            )}
          </li>
        ))}
      </ul>

      {/* Approve button — only visible to pending signers */}
      {walletIsSigner && !thresholdMet && (
        <div className="multisig-panel__action">
          {walletAlreadyApproved ? (
            <p className="multisig-panel__success">
              ✓ You have already approved this swap. Waiting for other signers.
            </p>
          ) : (
            <button
              className="multisig-panel__btn multisig-panel__btn--primary"
              onClick={handleApprove}
              disabled={approving || !canApprove}
              aria-busy={approving}
            >
              {approving ? (
                <>
                  <span className="multisig-panel__spinner" aria-hidden="true" />
                  Approving…
                </>
              ) : (
                <>🔏 Approve Swap</>
              )}
            </button>
          )}
        </div>
      )}

      {!walletIsSigner && !thresholdMet && (
        <p className="multisig-panel__info" style={{ marginBottom: 0 }}>
          Your wallet is not a configured signer. Waiting for authorised signers to approve.
        </p>
      )}

      {thresholdMet && (
        <p className="multisig-panel__success">
          ✓ All required approvals collected. The swap is now active.
        </p>
      )}

      {error && (
        <p className="multisig-panel__error" role="alert">{error}</p>
      )}
      {success && !error && (
        <p className="multisig-panel__success" role="status">{success}</p>
      )}
    </div>
  );
}
