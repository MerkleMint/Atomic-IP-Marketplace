export interface Listing {
  id: number;
  owner: string;
  ipfs_hash: string;
  merkle_root: string;
  royalty_bps: number;
  royalty_recipient: string;
  price_usdc: number;
  pendingSwaps?: any[]; // Attached by useMyListings
}
