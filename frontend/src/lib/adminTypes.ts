export type AdminRole = 'super_admin' | 'operator' | 'viewer';

export interface AdminUser {
  address: string;
  role: AdminRole;
  permissions: Permission[];
}

export type Permission = 
  | 'pause_contracts'
  | 'update_fees'
  | 'update_config'
  | 'resolve_disputes'
  | 'view_logs'
  | 'view_metrics'
  | 'manage_tokens'
  | 'transfer_admin'
  | 'manage_multisig';

export interface AdminConfig {
  fee_bps: number;
  fee_recipient: string;
  cancel_delay_secs: number;
  swap_expiry_secs: number;
  paused: boolean;
}

export interface TimelockOperation {
  id: string;
  type: 'pause' | 'unpause' | 'update_config' | 'update_fees' | 'resolve_dispute';
  targetContract: 'atomic_swap' | 'ip_registry';
  proposedBy: string;
  proposedAt: number;
  executeAt: number;
  status: 'pending' | 'executed' | 'cancelled' | 'expired';
  approvals: string[];
  requiredApprovals: number;
  data?: any;
}

export interface AuditLogEntry {
  id: string;
  timestamp: number;
  admin: string;
  action: string;
  targetContract: 'atomic_swap' | 'ip_registry';
  details: any;
  txHash?: string;
}

export interface SystemHealthMetrics {
  totalSwaps: number;
  activeSwaps: number;
  completedSwaps: number;
  disputedSwaps: number;
  totalListings: number;
  totalVolumeUsdc: number;
  contractPaused: boolean;
  lastBlockTimestamp: number;
  pendingTimelockOps: number;
}

// NOTE: fee-update governance state now lives entirely on-chain — see
// FeeProposal / GovernanceConfig in ./feeGovernanceTypes.ts, which mirror the
// atomic_swap contract's propose_fee_update / approve_fee_update /
// execute_fee_update flow.
