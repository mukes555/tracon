import { createContext, useCallback, useContext, useMemo, useRef, useState } from "react";
import { Toaster } from "../components/Toaster";

/// One vocabulary for everything the app tells the user.
///
/// The rule the whole app follows:
///   toast  the result of something the user just did (this file)
///   banner a condition that is true right now and stays until it changes
///          (StatusBanners: capture paused, recorder unreachable, an
///          update waiting, writes failing)
///   inline a control's own micro-state, like "Copied" on a copy button
export type ToastLevel = "success" | "error" | "info";
export type ToastAction = { label: string; run: () => void };

export type Toast = {
  id: number;
  level: ToastLevel;
  text: string;
  action?: ToastAction;
};

/// An error is worth reading twice; a confirmation is not.
const DISMISS_MS: Record<ToastLevel, number> = {
  success: 4000,
  info: 5000,
  error: 9000,
};

const MAX_VISIBLE = 3;

type Notify = {
  success: (text: string, action?: ToastAction) => void;
  error: (text: string, action?: ToastAction) => void;
  info: (text: string, action?: ToastAction) => void;
  dismiss: (id: number) => void;
};

const NotifyContext = createContext<Notify | null>(null);

export function NotifyProvider(props: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);
  const timers = useRef(new Map<number, number>());

  const dismiss = useCallback((id: number) => {
    const timer = timers.current.get(id);
    if (timer !== undefined) window.clearTimeout(timer);
    timers.current.delete(id);
    setToasts((list) => list.filter((t) => t.id !== id));
  }, []);

  const push = useCallback(
    (level: ToastLevel, text: string, action?: ToastAction) => {
      const id = nextId.current++;
      setToasts((list) => {
        // Flipping a switch twice should not stack two identical lines.
        const withoutTwin = list.filter((t) => !(t.text === text && t.level === level));
        return [...withoutTwin, { id, level, text, action }].slice(-MAX_VISIBLE);
      });
      timers.current.set(
        id,
        window.setTimeout(() => dismiss(id), DISMISS_MS[level]),
      );
    },
    [dismiss],
  );

  const value = useMemo<Notify>(
    () => ({
      success: (text, action) => push("success", text, action),
      error: (text, action) => push("error", text, action),
      info: (text, action) => push("info", text, action),
      dismiss,
    }),
    [push, dismiss],
  );

  return (
    <NotifyContext.Provider value={value}>
      {props.children}
      <Toaster toasts={toasts} onDismiss={dismiss} />
    </NotifyContext.Provider>
  );
}

export function useNotify(): Notify {
  const value = useContext(NotifyContext);
  if (!value) throw new Error("useNotify needs a NotifyProvider above it");
  return value;
}

/// Runs a write and reports it the same way everywhere: the success line
/// only once the recorder confirmed, a named failure otherwise.
export async function report(
  notify: Notify,
  work: Promise<unknown>,
  done: string,
  failed = "Could not save",
  onFail?: () => void,
) {
  try {
    await work;
    notify.success(done);
  } catch {
    onFail?.();
    notify.error(failed);
  }
}
