import { useEffect, useRef } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), summary, [tabindex]:not([tabindex="-1"])';

/// Modal focus handling for a dialog: focus the container on mount (it needs
/// tabIndex={-1}), keep Tab cycling inside it, and hand focus back to
/// whatever had it when the dialog closes.
export function useFocusTrap<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  useEffect(() => {
    const container = ref.current;
    if (!container) return;
    const previous = document.activeElement as HTMLElement | null;
    container.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      const focusables = container.querySelectorAll<HTMLElement>(FOCUSABLE);
      if (focusables.length === 0) {
        e.preventDefault();
        return;
      }
      const first = focusables[0];
      const last = focusables[focusables.length - 1];
      const active = document.activeElement;
      const leavingBackwards = e.shiftKey && (active === first || active === container);
      const leavingForwards = !e.shiftKey && active === last;
      if (leavingBackwards) {
        e.preventDefault();
        last.focus();
      } else if (leavingForwards) {
        e.preventDefault();
        first.focus();
      }
    };
    container.addEventListener("keydown", onKey);
    return () => {
      container.removeEventListener("keydown", onKey);
      // A detached element ignores focus(), so a row that scrolled out of the
      // list in the meantime is harmless.
      previous?.focus();
    };
  }, []);
  return ref;
}
