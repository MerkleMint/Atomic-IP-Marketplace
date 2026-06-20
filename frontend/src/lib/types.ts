export const USDC_DECIMALS = 7;

export interface Listing {
  id: number;
  owner: string;
  ipfs_hash: string;
  merkle_root: string;
  royalty_bps: number;
  royalty_recipient: string;
  price_usdc: number;
  pendingSwaps?: any[]; // Attached by useMyListings
  current_version?: number; // 0 or absent = never versioned
}

export interface IpVersion {
  version_number: number;
  timestamp: number;
  changelog: string;
  ipfs_hash: string;
  merkle_root: string;
  price_usdc: number;
  royalty_bps: number;
  created_by: string;
}
