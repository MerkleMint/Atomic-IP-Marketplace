import { useState, useEffect, useCallback, useRef } from "react";
import { getSwapsByBuyer, getSwapsBySeller, getSwap, getLedgerTimestamp } from "../lib/contractClient";

const POLL_INTERVAL_MS = 15_000;

export interface Swap {
  id: number;
  listing_id: number;
  buyer: string;
  seller: string;
  usdc_amount: number;
  usdc_token: string;
  created_at: number;
  expires_at: number;
  status: string;
  decryption_key: string | null;
}

export function useMySwaps(walletAddress: string | null) {
  const [swaps, setSwaps] = useState<Swap[]>([]);
  const [ledgerTimestamp, setLedgerTimestamp] = useState<number>(
    () => Math.floor(Date.now() / 1000)
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchSwaps = useCallback(async () => {
    if (!walletAddress) { setSwaps([]); return; }
    setLoading(true);
    setError(null);
    try {
      const [buyerIds, sellerIds, ts] = await Promise.all([
        getSwapsByBuyer(walletAddress),
        getSwapsBySeller(walletAddress).catch(() => [] as number[]),
        getLedgerTimestamp(),
      ]);
      setLedgerTimestamp(ts);
      // Deduplicate IDs (a wallet could theoretically be both buyer and seller)
      const allIds = [...new Set([...buyerIds, ...sellerIds])];
      if (allIds.length === 0) { setSwaps([]); return; }
      const results = await Promise.allSettled(allIds.map((id) => getSwap(id)));
      const loaded = results
        .filter((r): r is PromiseFulfilledResult<Swap> => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value);
      setSwaps(loaded);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load swaps.");
    } finally {
      setLoading(false);
    }
  }, [walletAddress]);

  useEffect(() => {
    fetchSwaps();
    timerRef.current = setInterval(fetchSwaps, POLL_INTERVAL_MS);

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
        fetchSwaps();
        timerRef.current = setInterval(fetchSwaps, POLL_INTERVAL_MS);
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [fetchSwaps]);

  return { swaps, ledgerTimestamp, loading, error, refresh: fetchSwaps };
}
