/// One line of Settings: what it is on the left, the control on the right.
/// Rows stack inside a card so a whole group reads as one block instead of
/// a page of separate panels.
export function SettingRow(props: {
  label: string;
  hint?: React.ReactNode;
  control: React.ReactNode;
  /// Puts the control under the text, for controls too wide to sit inline.
  stacked?: boolean;
}) {
  return (
    <div className={props.stacked ? "setting-row stacked" : "setting-row"}>
      <div className="setting-text">
        <span className="setting-label">{props.label}</span>
        {props.hint && <span className="setting-hint">{props.hint}</span>}
      </div>
      <div className="setting-control">{props.control}</div>
    </div>
  );
}
