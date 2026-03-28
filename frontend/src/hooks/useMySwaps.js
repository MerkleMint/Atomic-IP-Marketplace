import { useState, useEffect, useCallback, useRef } from "react";
import { getSwapsByBuyer, getSwap, getLedgerTimestamp } from "../lib/contractClient";

const POLL_INTERVAL_MS = 15_000; // re-fetch every 15 s

/**
 * useMySwaps
 *
 * Fetches all swaps for the connected buyer and keeps them fresh.
 *
 * @param {string|null} buyerAddress - Stellar public key, or null when disconnected
 * @returns {{
 *   swaps: object[],
 *   ledgerTimestamp: number,
 *   loading: boolean,
 *   error: string|null,
 *   refresh: () => void,
 * }}
 */
export function useMySwaps(buyerAddress) {
  const [swaps, setSwaps] = useState([]);
  const [ledgerTimestamp, setLedgerTimestamp] = useState(
    () => Math.floor(Date.now() / 1000)
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);
  const timerRef = useRef(null);

  const fetchSwaps = useCallback(async () => {
    if (!buyerAddress) {
      setSwaps([]);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const [ids, ts] = await Promise.all([
        getSwapsByBuyer(buyerAddress),
        getLedgerTimestamp(),
      ]);

      setLedgerTimestamp(ts);

      if (ids.length === 0) {
        setSwaps([]);
        return;
      }

      // Fetch all swap details in parallel, drop nulls (expired from ledger)
      const results = await Promise.allSettled(ids.map((id) => getSwap(id)));
      const loaded = results
        .filter((r) => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value);

      setSwaps(loaded);
    } catch (err) {
      setError(err.message || "Failed to load swaps.");
    } finally {
      setLoading(false);
    }
  }, [buyerAddress]);

  // Initial fetch + polling
  useEffect(() => {
    fetchSwaps();

    timerRef.current = setInterval(fetchSwaps, POLL_INTERVAL_MS);
    return () => clearInterval(timerRef.current);
  }, [fetchSwaps]);

  return { swaps, ledgerTimestamp, loading, error, refresh: fetchSwaps };
}
