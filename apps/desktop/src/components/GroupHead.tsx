/// Header for a group of rows: a colored bar, the label, a count pill, and
/// an optional action on the right.
export function GroupHead(props: {
  label: string;
  count: number;
  tone?: string;
  action?: React.ReactNode;
}) {
  return (
    <h2 className={props.tone ? `group-head sev-${props.tone}` : "group-head"}>
      <span className="group-bar" />
      {props.label}
      <span className="group-count">{props.count}</span>
      {props.action}
    </h2>
  );
}
