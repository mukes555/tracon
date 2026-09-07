/// The strip under a data view: what is shown, how fresh it is, and the
/// view's primary actions with their shortcuts.
export function StatusBar(props: { left: string; right?: React.ReactNode }) {
  return (
    <footer className="statusbar">
      <span>{props.left}</span>
      {props.right && <span className="statusbar-right">{props.right}</span>}
    </footer>
  );
}

/// The list shortcuts, shown on the right of a status bar.
export function KeyHints(props: { ack?: boolean }) {
  return (
    <span className="key-hints">
      <kbd className="kbd">↑</kbd>
      <kbd className="kbd">↓</kbd> move
      {props.ack && (
        <>
          <kbd className="kbd">A</kbd> acknowledge
        </>
      )}
      <kbd className="kbd">Esc</kbd> close
    </span>
  );
}
