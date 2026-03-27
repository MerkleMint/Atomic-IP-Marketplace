import { useState, useEffect, useCallback, useRef } from "react";
import {
  getListingsByOwner,
  getListing,
  getSwapsBySeller,
  getSwap,
} from "../lib/contractClient";

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
  const [listings, setListings] = useState<any[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const timerRef = useRef<any>(null);

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
        .filter((r: any) => r.status === "fulfilled" && r.value !== null)
        .map((r: any) => r.value);

      const allSellerSwaps = swapResults
        .filter((r: any) => r.status === "fulfilled" && r.value !== null)
        .map((r: any) => r.value)
        .filter((s) => s.status === "Pending");

      // Attach pending swaps to their respective listing
      const enriched = loadedListings.map((listing) => ({
        ...listing,
        pendingSwaps: allSellerSwaps.filter((s) => s.listing_id === listing.id),
      }));

      setListings(enriched);
    } catch (err: any) {
      setError(err.message || "Failed to load listings.");
    } finally {
      setLoading(false);
    }
  }, [ownerAddress]);

  useEffect(() => {
    fetchListings();
    timerRef.current = setInterval(fetchListings, POLL_INTERVAL_MS);
    return () => clearInterval(timerRef.current);
  }, [fetchListings]);

  return { listings, loading, error, refresh: fetchListings };
}
