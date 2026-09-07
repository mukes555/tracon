import type { Toast } from "../lib/notify";

/// The stack of transient messages, bottom center. Errors announce
/// themselves and carry a close button; the rest fade on their own.
export function Toaster(props: { toasts: Toast[]; onDismiss: (id: number) => void }) {
  if (props.toasts.length === 0) return null;
  return (
    <div className="toaster" aria-live="polite">
      {props.toasts.map((t) => (
        <div
          key={t.id}
          className={`toast ${t.level}`}
          role={t.level === "error" ? "alert" : "status"}
        >
          <span className="toast-dot" aria-hidden="true" />
          <span className="toast-text">{t.text}</span>
          {t.action && (
            <button
              className="toast-action"
              onClick={() => {
                t.action?.run();
                props.onDismiss(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
          <button className="toast-close" aria-label="Dismiss" onClick={() => props.onDismiss(t.id)}>
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
