/**
 * Fee-governance types.
 *
 * Mirrors the on-chain GovernanceConfig and FeeProposal structs. Fee changes
 * (fee_bps, fee_recipient, cancel_delay_secs) require on-chain quorum from
 * this signer set — there is no client-side proposal/approval state.
 */

export interface GovernanceConfig {
  /** Stellar addresses authorised to propose and approve fee updates. */
  signers: string[];
  /** Minimum number of distinct approvals required to reach quorum. */
  required_approvals: number;
}

export interface FeeProposal {
  proposal_id: number;
  fee_bps: number;
  fee_recipient: string;
  cancel_delay_secs: number;
  proposer: string;
  /** Governance signers that have approved this proposal so far. */
  approved_by: string[];
  created_at: number;
  /** Unix timestamp (seconds) quorum was first reached, or null if not yet. */
  quorum_reached_at: number | null;
  executed: boolean;
}

/** Minimum on-chain delay (seconds) between quorum and execute_fee_update — mirrors FEE_GOVERNANCE_TIMELOCK_SECS in the contract. */
export const FEE_GOVERNANCE_TIMELOCK_SECS = 86_400;
