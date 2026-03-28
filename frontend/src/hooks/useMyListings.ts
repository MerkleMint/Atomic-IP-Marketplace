import { useState, useEffect, useCallback, useRef } from "react";
import {
  getListingsByOwner,
  getListing,
  getSwapsBySeller,
  getSwap,
} from "../lib/contractClient";
import type { Listing } from "../lib/types";

const POLL_INTERVAL_MS = 15_000;

/**
 * useMyListings
 *
 * Fetches all IP listings owned by the connected wallet, along with any
 * pending swaps against each listing.
 *
 * @param {string|null} ownerAddress - Stellar public key, or null when disconnected
 * @returns {{
 *   listings: object[],   // each listing has a `pendingSwaps` array attached
 *   loading: boolean,
 *   error: string|null,
 *   refresh: () => void,
 * }}
 */
export function useMyListings(ownerAddress: string | null) {
  const [listings, setListings] = useState<Listing[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<NodeJS.Timeout | null>(null);

  const fetchListings = useCallback(async () => {
    if (!ownerAddress) {
      setListings([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Fetch listing IDs and all seller swap IDs in parallel
      const [listingIds, sellerSwapIds] = await Promise.all([
        getListingsByOwner(ownerAddress),
        getSwapsBySeller(ownerAddress).catch(() => []),
      ]);

      if (listingIds.length === 0) {
        setListings([]);
        return;
      }

      // Fetch listing details and all seller swaps in parallel
      const [listingResults, swapResults] = await Promise.all([
        Promise.allSettled(listingIds.map((id) => getListing(id))),
        Promise.allSettled(sellerSwapIds.map((id) => getSwap(id))),
      ]);

      const loadedListings = listingResults
        .filter(
          (r: PromiseFulfilledResult<Listing | null>) =>
            r.status === "fulfilled" && r.value !== null
        )
        .map((r: PromiseFulfilledResult<Listing>) => r.value) as Listing[];

      const allSellerSwaps = swapResults
        .filter(
          (r: PromiseFulfilledResult<any>) =>
            r.status === "fulfilled" && r.value !== null
        )
        .map((r: PromiseFulfilledResult<any>) => r.value)
        .filter((s: any) => s.status === "Pending");

      // Attach pending swaps to their respective listing
      const enriched: Listing[] = loadedListings.map((listing) => ({
        ...listing,
        pendingSwaps: allSellerSwaps.filter(
          (s: any) => s.listing_id === listing.id
        ),
      })) as Listing[];

      setListings(enriched);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "Failed to load listings.");
    } finally {
      setLoading(false);
    }
  }, [ownerAddress]);

  useEffect(() => {
    fetchListings();
    timerRef.current = setInterval(fetchListings, POLL_INTERVAL_MS);
    return () => clearInterval(timerRef.current);
  }, [fetchListings]);

  return {
    listings,
    loading,
    error,
    refresh: fetchListings,
  } as const;
}
