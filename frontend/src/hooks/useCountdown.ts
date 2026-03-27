import { useState, useEffect } from "react";

export function useCountdown(targetTimestamp: number) {
  const getRemaining = () =>
    Math.max(0, targetTimestamp - Math.floor(Date.now() / 1000));

  const [remaining, setRemaining] = useState<number>(getRemaining);

  useEffect(() => {
    if (remaining === 0) return;
    const interval = setInterval(() => {
      const next = getRemaining();
      setRemaining(next);
      if (next === 0) clearInterval(interval);
    }, 1000);
    return () => clearInterval(interval);
  }, [targetTimestamp]);

  const mm = String(Math.floor(remaining / 60)).padStart(2, "0");
  const ss = String(remaining % 60).padStart(2, "0");
  return { remaining, formatted: `${mm}:${ss}` };
}
