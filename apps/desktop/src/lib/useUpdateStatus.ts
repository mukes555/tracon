import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { UpdateStatus } from "./types";

// The Rust worker does the daily fetch; this only re-reads its cached
// answer, so a banner appears within half an hour of a check.
const REREAD_MS = 30 * 60 * 1000;

export function useUpdateStatus() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = () =>
      api
        .updateStatus()
        .then((s) => {
          if (!cancelled) setStatus(s);
        })
        .catch(() => {});
    load();
    const timer = window.setInterval(load, REREAD_MS);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const checkNow = useCallback(async () => {
    const next = await api.updateCheckNow();
    setStatus(next);
    return next;
  }, []);

  const setEnabled = useCallback(async (enabled: boolean) => {
    await api.setUpdateCheck(enabled);
    setStatus((s) => (s ? { ...s, enabled } : s));
  }, []);

  return { status, checkNow, setEnabled };
}
