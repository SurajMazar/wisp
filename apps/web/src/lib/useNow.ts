import { useEffect, useState } from "react";

/** Current time, re-rendering every `ms` while `active` (for live elapsed timers). */
export function useNow(active: boolean, ms = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setNow(Date.now()), ms);
    return () => clearInterval(t);
  }, [active, ms]);
  return now;
}

export const seconds = (ms: number) => `${Math.max(0, Math.round(ms / 1000))}s`;
