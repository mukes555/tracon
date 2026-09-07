import { useCallback, useEffect, useRef, useState } from "react";

const NOTE_MS = 2500;

export type NoteTone = "ok" | "bad";
export type TransientNote = { text: string; tone: NoteTone };

/// A control's own micro-state, like "Copied" on a copy button. Anything
/// the user should read as a result of an action goes through notify()
/// instead; anything that stays true goes in a banner. See lib/notify.tsx.
export function useTransientNote(ms = NOTE_MS) {
  const [note, setNote] = useState<TransientNote | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const show = useCallback(
    (text: string, tone: NoteTone = "ok") => {
      window.clearTimeout(timer.current);
      setNote({ text, tone });
      timer.current = window.setTimeout(() => setNote(null), ms);
    },
    [ms],
  );
  useEffect(() => () => window.clearTimeout(timer.current), []);
  return { note, show };
}
