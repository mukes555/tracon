/// The strip under a data view: what is shown, how fresh it is, and the
/// view's primary actions with their shortcuts.
export function StatusBar(props: { left: string; right?: React.ReactNode }) {
  return (
    <footer className="statusbar">
      <span className="statusbar-left">{props.left}</span>
      {props.right && <span className="statusbar-right">{props.right}</span>}
    </footer>
  );
}
