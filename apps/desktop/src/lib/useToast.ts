import { useCallback, useEffect, useRef, useState } from "react";

const TOAST_MS = 6000;

export type Toast = { text: string; undo?: () => void };

/// One toast at a time with an optional undo; a new one replaces the old.
export function useToast() {
  const [toast, setToast] = useState<Toast | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const show = useCallback((text: string, undo?: () => void) => {
    window.clearTimeout(timer.current);
    setToast({ text, undo });
    timer.current = window.setTimeout(() => setToast(null), TOAST_MS);
  }, []);
  const dismiss = useCallback(() => {
    window.clearTimeout(timer.current);
    setToast(null);
  }, []);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return { toast, show, dismiss };
}
