import { agentLabel } from "../lib/format";

/** Filter chips scoping a list to one agent; hidden when only one agent exists. */
export function AgentChips(props: {
  counts: [string, number][];
  value: string;
  onChange: (agent: string) => void;
}) {
  if (props.counts.length <= 1) return null;
  const total = props.counts.reduce((sum, [, n]) => sum + n, 0);
  return (
    <div className="filter-chips agent-chips">
      <button
        className={props.value === "all" ? "chip active" : "chip"}
        onClick={() => props.onChange("all")}
      >
        All agents <span className="chip-count">{total}</span>
      </button>
      {props.counts.map(([agent, n]) => (
        <button
          key={agent}
          className={props.value === agent ? "chip active" : "chip"}
          onClick={() => props.onChange(agent)}
        >
          {agentLabel(agent)} <span className="chip-count">{n}</span>
        </button>
      ))}
    </div>
  );
}
