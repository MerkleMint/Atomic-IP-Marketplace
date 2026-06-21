/**
 * Multi-signature approval types.
 *
 * Mirrors the on-chain MultiSigConfig and MultiSigApproval structs.
 */

export interface MultiSigConfig {
  /** USDC amount (7-decimal i128 as string) at or above which multi-sig is required. */
  threshold: string;
  /** Stellar addresses that may approve high-value swaps. */
  signers: string[];
  /** Minimum approvals needed to unblock a swap (2 for 2-of-2 or 2-of-3). */
  required_approvals: number;
  /** When false the multi-sig gate is bypassed entirely. */
  enabled: boolean;
}

export interface MultiSigApproval {
  swap_id: number;
  /** Addresses that have already approved this swap. */
  approved_by: string[];
  /** Current nonce — increments after each approval to prevent replay. */
  nonce: number;
}

export type SignerStatus = "approved" | "pending" | "unknown";

export interface SignerInfo {
  address: string;
  status: SignerStatus;
}

/** Human-readable USDC amount for the default threshold (10,000 USDC). */
export const DEFAULT_MULTISIG_THRESHOLD_USDC = 10_000;
export const USDC_DECIMALS = 7;

/** Convert a human-readable USDC amount to the 7-decimal i128 string. */
export function usdcToRaw(usdc: number): string {
  return String(Math.round(usdc * Math.pow(10, USDC_DECIMALS)));
}

/** Convert a raw 7-decimal i128 string to human-readable USDC. */
export function rawToUsdc(raw: string): number {
  return Number(raw) / Math.pow(10, USDC_DECIMALS);
}
