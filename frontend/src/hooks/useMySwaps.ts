import { useState, useEffect, useCallback, useRef } from "react";
import { getSwapsByBuyer, getSwap, getLedgerTimestamp } from "../lib/contractClient";

const POLL_INTERVAL_MS = 15_000;

export interface Swap {
  id: number;
  listing_id: number;
  buyer: string;
  seller: string;
  usdc_amount: number;
  created_at: number;
  expires_at: number;
  status: string;
  decryption_key: string | null;
}

export function useMySwaps(buyerAddress: string | null) {
  const [swaps, setSwaps] = useState<Swap[]>([]);
  const [ledgerTimestamp, setLedgerTimestamp] = useState<number>(
    () => Math.floor(Date.now() / 1000)
  );
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchSwaps = useCallback(async () => {
    if (!buyerAddress) { setSwaps([]); return; }
    setLoading(true);
    setError(null);
    try {
      const [ids, ts] = await Promise.all([
        getSwapsByBuyer(buyerAddress),
        getLedgerTimestamp(),
      ]);
      setLedgerTimestamp(ts);
      if (ids.length === 0) { setSwaps([]); return; }
      const results = await Promise.allSettled(ids.map((id) => getSwap(id)));
      const loaded = results
        .filter((r): r is PromiseFulfilledResult<Swap> => r.status === "fulfilled" && r.value !== null)
        .map((r) => r.value);
      setSwaps(loaded);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load swaps.");
    } finally {
      setLoading(false);
    }
  }, [buyerAddress]);

  useEffect(() => {
    fetchSwaps();
    timerRef.current = setInterval(fetchSwaps, POLL_INTERVAL_MS);
    return () => { if (timerRef.current) clearInterval(timerRef.current); };
  }, [fetchSwaps]);

  return { swaps, ledgerTimestamp, loading, error, refresh: fetchSwaps };
}
