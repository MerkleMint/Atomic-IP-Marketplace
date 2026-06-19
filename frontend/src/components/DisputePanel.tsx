import { useState, useEffect, useCallback } from "react";
import type { Wallet } from "../lib/walletKit";
import type { Swap } from "../hooks/useMySwaps";
import {
  getDispute,
  getEvidence,
  raiseDispute,
  submitEvidence,
  commitVote,
  revealVote,
  finalizeDispute,
  appealDispute,
  getCurrentLedger,
  type DisputeRecord,
  type EvidenceItem,
} from "../lib/contractClient";
import "./DisputePanel.css";

interface Props {
  swap: Swap;
  wallet: Wallet;
  onSwapUpdated: () => void;
}

type Phase = "commit" | "reveal" | "closed";

function getPhase(dispute: DisputeRecord, currentLedger: number): Phase {
  if (currentLedger <= dispute.commit_deadline_ledger) return "commit";
  if (currentLedger <= dispute.reveal_deadline_ledger) return "reveal";
  return "closed";
}

function outcomeLabel(outcome: DisputeRecord["outcome"]): string {
  if (outcome === "FavorBuyer") return "Resolved: Buyer";
  if (outcome === "FavorSeller") return "Resolved: Seller";
  return "Pending";
}

function shortAddr(addr: string) {
  return addr ? `${addr.slice(0, 4)}…${addr.slice(-4)}` : "";
}

export function DisputePanel({ swap, wallet, onSwapUpdated }: Props) {
  const isBuyer = wallet.address === swap.buyer;
  const isDisputed = swap.status === "Disputed";
  const isCompleted = swap.status === "Completed";
  const isResolved =
    swap.status === "ResolvedBuyer" || swap.status === "ResolvedSeller";

  const [dispute, setDispute] = useState<DisputeRecord | null>(null);
  const [evidence, setEvidence] = useState<EvidenceItem[]>([]);
  const [currentLedger, setCurrentLedger] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  // Evidence form
  const [ipfsInput, setIpfsInput] = useState("");

  // Arbiter vote commit form
  const [voteChoice, setVoteChoice] = useState<"buyer" | "seller">("buyer");
  const [voteSalt, setVoteSalt] = useState("");

  // Arbiter vote reveal form
  const [revealChoice, setRevealChoice] = useState<"buyer" | "seller">("buyer");
  const [revealSalt, setRevealSalt] = useState("");

  const loadDispute = useCallback(async () => {
    if (!isDisputed && !isResolved) return;
    try {
      const [d, ledger] = await Promise.all([
        getDispute(swap.id),
        getCurrentLedger(),
      ]);
      setDispute(d);
      setCurrentLedger(ledger);
      if (d) {
        const items: EvidenceItem[] = [];
        for (let i = 0; i < d.evidence_count; i++) {
          const ev = await getEvidence(swap.id, i);
          if (ev) items.push(ev);
        }
        setEvidence(items);
      }
    } catch {
      // non-fatal
    }
  }, [swap.id, isDisputed, isResolved]);

  useEffect(() => {
    loadDispute();
  }, [loadDispute]);

  function notify(msg: string) {
    setSuccess(msg);
    setError(null);
    setTimeout(() => setSuccess(null), 4000);
  }

  async function handleRaiseDispute() {
    setLoading(true);
    setError(null);
    try {
      await raiseDispute(swap.id, wallet);
      notify("Dispute raised. Arbiters may now submit votes.");
      onSwapUpdated();
    } catch (e: any) {
      setError(e?.message ?? "Failed to raise dispute.");
    } finally {
      setLoading(false);
    }
  }

  async function handleSubmitEvidence() {
    if (!ipfsInput.trim()) {
      setError("Enter an IPFS hash.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await submitEvidence(swap.id, ipfsInput.trim(), wallet);
      notify(`Evidence submitted: ${ipfsInput.trim()}`);
      setIpfsInput("");
      await loadDispute();
    } catch (e: any) {
      setError(e?.message ?? "Failed to submit evidence.");
    } finally {
      setLoading(false);
    }
  }

  async function handleCommitVote() {
    if (!voteSalt.trim()) {
      setError("Enter a secret salt. Save it — you'll need it for reveal.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await commitVote(swap.id, voteChoice === "buyer", voteSalt.trim(), wallet);
      notify("Vote committed. Keep your salt safe for the reveal phase.");
      setVoteSalt("");
    } catch (e: any) {
      setError(e?.message ?? "Failed to commit vote.");
    } finally {
      setLoading(false);
    }
  }

  async function handleRevealVote() {
    if (!revealSalt.trim()) {
      setError("Enter the salt you used during the commit phase.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      await revealVote(
        swap.id,
        revealChoice === "buyer",
        revealSalt.trim(),
        wallet
      );
      notify("Vote revealed. Weights updated.");
      setRevealSalt("");
      await loadDispute();
    } catch (e: any) {
      setError(e?.message ?? "Failed to reveal vote.");
    } finally {
      setLoading(false);
    }
  }

  async function handleFinalize() {
    setLoading(true);
    setError(null);
    try {
      await finalizeDispute(swap.id, wallet);
      notify("Dispute finalized. Funds distributed.");
      onSwapUpdated();
      await loadDispute();
    } catch (e: any) {
      setError(e?.message ?? "Failed to finalize dispute.");
    } finally {
      setLoading(false);
    }
  }

  async function handleAppeal() {
    setLoading(true);
    setError(null);
    try {
      await appealDispute(swap.id, wallet);
      notify("Appeal submitted. An admin will review.");
      await loadDispute();
    } catch (e: any) {
      setError(e?.message ?? "Failed to submit appeal.");
    } finally {
      setLoading(false);
    }
  }

  // Panel only renders when there is something dispute-related to show
  if (!isDisputed && !isCompleted && !isResolved) return null;

  const phase = dispute ? getPhase(dispute, currentLedger) : null;
  const canFinalize =
    dispute &&
    phase === "closed" &&
    dispute.outcome === "Pending" &&
    isDisputed;
  const canAppeal =
    dispute &&
    dispute.outcome !== "Pending" &&
    !dispute.is_appealed &&
    isBuyer &&
    dispute.appeal_deadline_ledger != null &&
    currentLedger <= dispute.appeal_deadline_ledger;

  const totalWeight =
    dispute && dispute.outcome !== "Pending"
      ? BigInt(dispute.vote_weight_buyer) + BigInt(dispute.vote_weight_seller)
      : BigInt(dispute?.vote_weight_buyer ?? 0) +
        BigInt(dispute?.vote_weight_seller ?? 0);

  const buyerPct =
    totalWeight > 0n
      ? Number((BigInt(dispute?.vote_weight_buyer ?? 0) * 100n) / totalWeight)
      : 50;
  const sellerPct = 100 - buyerPct;

  const panelClass = [
    "dispute-panel",
    isDisputed && "dispute-panel--disputed",
    swap.status === "ResolvedBuyer" && "dispute-panel--resolved-buyer",
    swap.status === "ResolvedSeller" && "dispute-panel--resolved-seller",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={panelClass}>
      <div className="dispute-panel__header">
        <span className="dispute-panel__title">Dispute</span>
        {dispute && (
          <span
            className={`dispute-panel__outcome-badge dispute-panel__outcome-badge--${
              dispute.outcome === "Pending"
                ? "pending"
                : dispute.outcome === "FavorBuyer"
                ? "buyer"
                : "seller"
            }`}
          >
            {outcomeLabel(dispute.outcome)}
          </span>
        )}
      </div>

      {/* ── Raise dispute (buyer, while swap is Completed) ── */}
      {isCompleted && isBuyer && !dispute && (
        <div className="dispute-actions">
          <p style={{ fontSize: "0.82rem", color: "#64748b", margin: "0 0 8px" }}>
            If the delivered content does not match the listing, raise a dispute
            before the window closes.
          </p>
          <button
            className="dispute-btn dispute-btn--danger"
            onClick={handleRaiseDispute}
            disabled={loading}
          >
            {loading ? "Raising…" : "Raise Dispute"}
          </button>
        </div>
      )}

      {/* ── Active dispute view ── */}
      {dispute && (
        <>
          {/* Timeline */}
          <DisputeTimeline
            dispute={dispute}
            currentLedger={currentLedger}
            phase={phase}
          />

          {/* Vote weight bar (once any votes are in) */}
          {(BigInt(dispute.vote_weight_buyer) > 0n ||
            BigInt(dispute.vote_weight_seller) > 0n) && (
            <div className="dispute-votes">
              <div className="dispute-votes__label">Vote weights</div>
              <div className="dispute-votes__bar">
                <div
                  className="dispute-votes__bar-buyer"
                  style={{ width: `${buyerPct}%` }}
                />
                <div
                  className="dispute-votes__bar-seller"
                  style={{ width: `${sellerPct}%` }}
                />
              </div>
              <div className="dispute-votes__legend">
                <span className="dispute-votes__legend-buyer">
                  Buyer {dispute.vote_weight_buyer}
                </span>
                <span className="dispute-votes__legend-seller">
                  Seller {dispute.vote_weight_seller}
                </span>
              </div>
            </div>
          )}

          {/* Evidence list */}
          {evidence.length > 0 && (
            <div className="dispute-evidence">
              <div className="dispute-evidence__title">
                Evidence ({evidence.length})
              </div>
              <ul className="dispute-evidence__list">
                {evidence.map((ev, i) => (
                  <li key={i} className="dispute-evidence__item">
                    <div>
                      <div className="dispute-evidence__hash">{ev.ipfs_hash}</div>
                      <div className="dispute-evidence__submitter">
                        by {shortAddr(ev.submitter)} · ledger{" "}
                        {ev.submitted_at_ledger}
                      </div>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div className="dispute-actions">
            {/* Evidence submission (buyer or seller while disputed) */}
            {isDisputed && phase !== "closed" && (
              <div className="dispute-action-group">
                <div className="dispute-action-group__title">Submit Evidence</div>
                <div className="dispute-form">
                  <div className="dispute-form__row">
                    <input
                      className="dispute-form__input"
                      placeholder="IPFS hash (e.g. QmXyz…)"
                      value={ipfsInput}
                      onChange={(e) => setIpfsInput(e.target.value)}
                    />
                    <button
                      className="dispute-btn dispute-btn--neutral"
                      onClick={handleSubmitEvidence}
                      disabled={loading}
                    >
                      {loading ? "…" : "Submit"}
                    </button>
                  </div>
                </div>
              </div>
            )}

            {/* Arbiter commit vote */}
            {isDisputed && phase === "commit" && (
              <div className="dispute-action-group">
                <div className="dispute-action-group__title">
                  Commit Vote (Arbiter)
                </div>
                <div className="dispute-form">
                  <div className="dispute-form__row">
                    <select
                      className="dispute-form__select"
                      value={voteChoice}
                      onChange={(e) =>
                        setVoteChoice(e.target.value as "buyer" | "seller")
                      }
                    >
                      <option value="buyer">Favour Buyer</option>
                      <option value="seller">Favour Seller</option>
                    </select>
                    <input
                      className="dispute-form__input"
                      placeholder="Secret salt"
                      value={voteSalt}
                      onChange={(e) => setVoteSalt(e.target.value)}
                    />
                    <button
                      className="dispute-btn dispute-btn--primary"
                      onClick={handleCommitVote}
                      disabled={loading}
                    >
                      {loading ? "…" : "Commit"}
                    </button>
                  </div>
                  <span className="dispute-form__hint">
                    Save your salt — you must provide it again during the reveal
                    phase.
                  </span>
                </div>
              </div>
            )}

            {/* Arbiter reveal vote */}
            {isDisputed && phase === "reveal" && (
              <div className="dispute-action-group">
                <div className="dispute-action-group__title">
                  Reveal Vote (Arbiter)
                </div>
                <div className="dispute-form">
                  <div className="dispute-form__row">
                    <select
                      className="dispute-form__select"
                      value={revealChoice}
                      onChange={(e) =>
                        setRevealChoice(e.target.value as "buyer" | "seller")
                      }
                    >
                      <option value="buyer">Favour Buyer</option>
                      <option value="seller">Favour Seller</option>
                    </select>
                    <input
                      className="dispute-form__input"
                      placeholder="Salt used at commit time"
                      value={revealSalt}
                      onChange={(e) => setRevealSalt(e.target.value)}
                    />
                    <button
                      className="dispute-btn dispute-btn--success"
                      onClick={handleRevealVote}
                      disabled={loading}
                    >
                      {loading ? "…" : "Reveal"}
                    </button>
                  </div>
                </div>
              </div>
            )}

            {/* Finalize (anyone, after reveal window) */}
            {canFinalize && (
              <div className="dispute-action-group">
                <div className="dispute-action-group__title">
                  Finalize Dispute
                </div>
                <p style={{ fontSize: "0.78rem", color: "#64748b", margin: "0 0 8px" }}>
                  The reveal window has closed. Finalize to distribute funds
                  according to arbiter vote weights.
                </p>
                <button
                  className="dispute-btn dispute-btn--warning"
                  onClick={handleFinalize}
                  disabled={loading}
                >
                  {loading ? "Finalizing…" : "Finalize Dispute"}
                </button>
              </div>
            )}

            {/* Appeal (buyer, after resolution, within window) */}
            {canAppeal && (
              <div className="dispute-action-group">
                <div className="dispute-action-group__title">Appeal</div>
                <p style={{ fontSize: "0.78rem", color: "#64748b", margin: "0 0 8px" }}>
                  You may appeal this outcome for admin review.
                </p>
                <button
                  className="dispute-btn dispute-btn--danger"
                  onClick={handleAppeal}
                  disabled={loading}
                >
                  {loading ? "Appealing…" : "Appeal Outcome"}
                </button>
              </div>
            )}

            {/* Appealed notice */}
            {dispute.is_appealed && (
              <div className="dispute-panel__appealed-notice">
                This dispute has been appealed and is pending admin review.
              </div>
            )}
          </div>
        </>
      )}

      {error && <div className="dispute-panel__error">{error}</div>}
      {success && <div className="dispute-panel__success">{success}</div>}
    </div>
  );
}

// ── Sub-component: Timeline ────────────────────────────────────────────────────

interface TimelineProps {
  dispute: DisputeRecord;
  currentLedger: number;
  phase: Phase | null;
}

function DisputeTimeline({ dispute, currentLedger, phase }: TimelineProps) {
  type Step = {
    label: string;
    ledger?: number;
    status: "done" | "active" | "future";
  };

  const steps: Step[] = [
    {
      label: "Dispute raised",
      ledger: dispute.raised_at_ledger,
      status: "done",
    },
    {
      label: `Commit window (closes ledger ${dispute.commit_deadline_ledger})`,
      ledger: dispute.commit_deadline_ledger,
      status:
        phase === "commit" ? "active" : currentLedger > dispute.commit_deadline_ledger ? "done" : "future",
    },
    {
      label: `Reveal window (closes ledger ${dispute.reveal_deadline_ledger})`,
      ledger: dispute.reveal_deadline_ledger,
      status:
        phase === "reveal" ? "active" : currentLedger > dispute.reveal_deadline_ledger ? "done" : "future",
    },
    {
      label:
        dispute.outcome !== "Pending"
          ? `Finalized at ledger ${dispute.resolved_at_ledger ?? "—"}`
          : "Finalize (after reveal window)",
      status: dispute.outcome !== "Pending" ? "done" : "future",
    },
  ];

  if (dispute.appeal_deadline_ledger != null) {
    steps.push({
      label: dispute.is_appealed
        ? "Appealed — awaiting admin review"
        : `Appeal window (closes ledger ${dispute.appeal_deadline_ledger})`,
      ledger: dispute.appeal_deadline_ledger,
      status:
        dispute.is_appealed
          ? "active"
          : currentLedger > dispute.appeal_deadline_ledger
          ? "done"
          : "future",
    });
  }

  return (
    <ul className="dispute-timeline">
      {steps.map((step, i) => (
        <li key={i} className="dispute-timeline__item">
          <div className={`dispute-timeline__dot dispute-timeline__dot--${step.status}`} />
          <span className="dispute-timeline__label">{step.label}</span>
          {step.ledger != null && (
            <span className="dispute-timeline__ledger">#{step.ledger}</span>
          )}
        </li>
      ))}
    </ul>
  );
}
