import { useState, useEffect, useCallback } from "react";

export function useCountdown(targetTimestamp: number) {
  // useCallback ensures the interval always closes over the current targetTimestamp.
  // Without this, the interval set on the first render would use a stale closure
  // and ignore any subsequent changes to targetTimestamp.
  const getRemaining = useCallback(
    () => Math.max(0, targetTimestamp - Math.floor(Date.now() / 1000)),
    [targetTimestamp]
  );

  const [remaining, setRemaining] = useState<number>(getRemaining);

  useEffect(() => {
    // Reset immediately when targetTimestamp changes so the displayed value is
    // correct before the first interval tick fires.
    setRemaining(getRemaining());
    if (getRemaining() === 0) return;
    const interval = setInterval(() => {
      const next = getRemaining();
      setRemaining(next);
      if (next === 0) clearInterval(interval);
    }, 1000);
    return () => clearInterval(interval);
  }, [getRemaining]); // getRemaining changes identity only when targetTimestamp changes

  const mm = String(Math.floor(remaining / 60)).padStart(2, "0");
  const ss = String(remaining % 60).padStart(2, "0");
  return { remaining, formatted: `${mm}:${ss}` };
}
