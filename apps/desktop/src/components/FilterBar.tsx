import type { KindFilter } from "../lib/types";

const FILTERS: { key: KindFilter; label: string }[] = [
  { key: "all", label: "All" },
  { key: "commands", label: "Commands" },
  { key: "files", label: "Files" },
  { key: "packages", label: "Packages" },
  { key: "prompts", label: "Prompts" },
  { key: "flagged", label: "Flagged" },
];

export function FilterBar(props: {
  query: string;
  onQuery: (q: string) => void;
  kind: KindFilter;
  onKind: (k: KindFilter) => void;
  right?: React.ReactNode;
}) {
  return (
    <div className="filterbar">
      <input
        className="search"
        type="search"
        placeholder="Search events..."
        value={props.query}
        onChange={(e) => props.onQuery(e.target.value)}
      />
      <div className="filter-chips">
        {FILTERS.map((f) => (
          <button
            key={f.key}
            className={props.kind === f.key ? "chip active" : "chip"}
            onClick={() => props.onKind(f.key)}
          >
            {f.label}
          </button>
        ))}
      </div>
      {props.right}
    </div>
  );
}
