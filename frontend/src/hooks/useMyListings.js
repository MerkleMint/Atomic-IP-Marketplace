import { useState, useEffect, useCallback, useRef } from "react";
import { getListingIdsByOwner, getListing, getSwapsBySeller, getSwap } from "../lib/contractClient";

const POLL_INTERVAL_MS = 15_000;

/**
 * useMyListings
 *
 * Fetches all IP listings owned by the connected seller, enriched with any
 * pending swaps against each listing.
 *
 * @param {string|null} sellerAddress - Stellar public key, or null when disconnected
 * @returns {{
 *   listings: object[],   // each listing has an `activeSwaps` array attached
 *   loading: boolean,
 *   error: string|null,
 *   refresh: () => void,
 * }}
 */
export function useMyListings(sellerAddress) {
  const [listings, setListings] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const timerRef = useRef(null);

  const fetchListings = useCallback(async () => {
    if (!sellerAddress) {
      setListings([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // 1. Get all listing IDs owned by this seller
      const [listingIds, sellerSwapIds] = await Promise.all([
        getListingIdsByOwner(sellerAddress),
        getSwapsBySeller(sellerAddress),
      ]);

      if (listingIds.length === 0) {
        setListings([]);
        return;
      }

      // 2. Fetch listing details + all seller swaps in parallel
      const [listingResults, swapResults] = await Promise.all([
        Promise.allSettled(listingIds.map((id) => getListing(id))),
        Promise.allSettled(sellerSwapIds.map((id) => getSwap(id))),
      ]);

      // 3. Build a map of listingId → pending swaps
      const swapsByListing = {};
      swapResults
        .filter((r) => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value)
        .filter((swap) => swap.status === "Pending")
        .forEach((swap) => {
          const lid = swap.listing_id;
          if (!swapsByListing[lid]) swapsByListing[lid] = [];
          swapsByListing[lid].push(swap);
        });

      // 4. Attach activeSwaps to each listing
      const loaded = listingResults
        .filter((r) => r.status === "fulfilled" && r.value !== null)
        .map((r) => ({
          ...r.value,
          activeSwaps: swapsByListing[r.value.id] ?? [],
        }));

      setListings(loaded);
    } catch (err) {
      setError(err.message || "Failed to load listings.");
    } finally {
      setLoading(false);
    }
  }, [sellerAddress]);

  useEffect(() => {
    fetchListings();
    timerRef.current = setInterval(fetchListings, POLL_INTERVAL_MS);
    return () => clearInterval(timerRef.current);
  }, [fetchListings]);

  return { listings, loading, error, refresh: fetchListings };
}
