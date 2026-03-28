import { useState, useEffect, useCallback, useRef } from "react";
import {
  getListingsByOwner,
  getListing,
  getSwapsBySeller,
  getSwap,
} from "../lib/contractClient";

const POLL_INTERVAL_MS = 15_000; // re-fetch every 15 s

/**
 * useMyListings
 *
 * Fetches all listings for the connected seller and their pending swaps.
 * Keeps them fresh via polling.
 *
 * @param {string|null} sellerAddress - Stellar public key, or null when disconnected
 * @returns {{
 *   listings: object[],
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
      // Get all listing IDs for this seller
      const listingIds = await getListingsByOwner(sellerAddress);

      if (listingIds.length === 0) {
        setListings([]);
        return;
      }

      // Fetch full listing details in parallel
      const listingResults = await Promise.allSettled(
        listingIds.map((id) => getListing(id))
      );

      // Filter out failures and nulls
      const loadedListings = listingResults
        .filter((r) => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value);

      // Get all swaps for this seller
      const swapIds = await getSwapsBySeller(sellerAddress);

      // For each swap, fetch details and cross-reference with listings
      const swapResults = await Promise.allSettled(
        swapIds.map((id) => getSwap(id))
      );

      const swaps = swapResults
        .filter((r) => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value);

      // Build a map of listing_id -> active swap (only pending)
      const activeSwapsByListing = {};
      swaps.forEach((swap) => {
        if (swap.status === "Pending" && !activeSwapsByListing[swap.listing_id]) {
          activeSwapsByListing[swap.listing_id] = swap;
        }
      });

      // Enrich listings with their active swap
      const enrichedListings = loadedListings.map((listing) => ({
        ...listing,
        active_swap: activeSwapsByListing[listing.id] || null,
      }));

      setListings(enrichedListings);
    } catch (err) {
      setError(err.message || "Failed to load listings.");
    } finally {
      setLoading(false);
    }
  }, [sellerAddress]);

  // Initial fetch + polling
  useEffect(() => {
    fetchListings();

    timerRef.current = setInterval(fetchListings, POLL_INTERVAL_MS);

    // Optimization: pause polling when the tab is hidden to avoid unnecessary RPC calls
    // and reduce battery consumption on mobile devices
    const handleVisibilityChange = () => {
      if (document.hidden) {
        if (timerRef.current) {
          clearInterval(timerRef.current);
          timerRef.current = null;
        }
      } else {
        // Tab became visible again, fetch immediately and restart polling
        fetchListings();
        timerRef.current = setInterval(fetchListings, POLL_INTERVAL_MS);
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [fetchListings]);

  return { listings, loading, error, refresh: fetchListings };
}
