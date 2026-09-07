import { useEffect, useRef, useState } from "react";

// How long the button stays armed before it quietly returns to its label.
const ARMED_MS = 5000;

/// A destructive action that asks once, in place: the first click swaps the
/// label for a question, the second click within five seconds runs it. No
/// dialog, no focus juggling, and nothing happens on a stray click.
export function ConfirmButton(props: {
  label: string;
  confirmLabel: string;
  onConfirm: () => void;
  className?: string;
}) {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  const onClick = () => {
    if (armed) {
      window.clearTimeout(timer.current);
      setArmed(false);
      props.onConfirm();
      return;
    }
    setArmed(true);
    timer.current = window.setTimeout(() => setArmed(false), ARMED_MS);
  };

  const base = props.className ?? "ack-btn";
  return (
    <button className={armed ? `${base} danger armed` : `${base} danger`} onClick={onClick}>
      {armed ? props.confirmLabel : props.label}
    </button>
  );
}
