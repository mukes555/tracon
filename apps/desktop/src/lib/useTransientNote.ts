import { useCallback, useEffect, useRef, useState } from "react";

const NOTE_MS = 2500;

export type NoteTone = "ok" | "bad";
export type TransientNote = { text: string; tone: NoteTone };

/// A short inline confirmation ("Theme saved", "copied") that clears itself.
/// Showing a new note restarts the timer; unmounting clears it so a late
/// timeout never touches a gone component.
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
