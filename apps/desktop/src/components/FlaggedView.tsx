import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import type { AgentEvent } from "../lib/types";
import { agentCounts, projectName } from "../lib/format";
import { type Category, categoryOf, type Severity, severityOf } from "../lib/flags";
import { AgentChips } from "./AgentChips";
import { EventRow } from "./EventRow";
import { GroupHead } from "./GroupHead";
import { FlagIcon } from "./icons";
import { Mascot } from "./Mascot";
import { KeyHints, StatusBar } from "./StatusBar";

const CATEGORY_LABELS: { key: Category | "all"; label: string }[] = [
  { key: "all", label: "All" },
  { key: "deletes", label: "Deletes" },
  { key: "pipe", label: "Pipe to shell" },
  { key: "credentials", label: "Credentials" },
  { key: "force-push", label: "Force push" },
  { key: "bypass", label: "Permission bypass" },
  { key: "packages", label: "Packages" },
  { key: "other", label: "Other" },
];

// Triage order: what can leak or destroy first, then the rest.
const SEVERITY_LABELS: { severity: Severity; label: string }[] = [
  { severity: "critical", label: "Critical" },
  { severity: "warning", label: "Warning" },
  { severity: "notice", label: "Notice" },
];

// Rendering hundreds of rows at once is what makes the view feel heavy.
const INITIAL_ROWS = 120;
const NO_EVENTS: AgentEvent[] = [];

type Row = { event: AgentEvent; category: Category; severity: Severity };
// null while loading, "error" when the fetch failed.
type AckedList = AgentEvent[] | null | "error";

export function FlaggedView(props: {
  flagged: AgentEvent[];
  ackedCount: number;
  /// Bumps whenever flags change through the UI (ack, reopen, undo).
  flagsVersion: number;
  selectedId?: number;
  onAck: (event: AgentEvent, acked: boolean) => Promise<void>;
  onAckMany: (events: AgentEvent[], acked: boolean) => Promise<void>;
  onOpenEvent: (event: AgentEvent, acked?: boolean) => void;
  /// The rows on screen in display order, so keyboard stepping matches.
  onVisibleRows: (events: AgentEvent[], acked: boolean) => void;
}) {
  const [bucket, setBucket] = useState<"open" | "acked">("open");
  const [ackedList, setAckedList] = useState<AckedList>(null);
  const [category, setCategory] = useState<Category | "all">("all");
  const [agent, setAgent] = useState("all");
  const [query, setQuery] = useState("");
  const [limit, setLimit] = useState(INITIAL_ROWS);
  const showingAcked = bucket === "acked";

  // Any change to the open list (ack, reopen, undo) can change the acked
  // list too, so it refetches on the counter and the version, never on the
  // open array's identity (which changes every poll). The previous list
  // stays on screen while the refetch runs; only the first load shows the
  // skeleton.
  useEffect(() => {
    if (!showingAcked) return;
    let stale = false;
    api
      .flaggedEvents(true)
      .then((list) => {
        if (!stale) setAckedList(list);
      })
      .catch(() => {
        if (!stale) setAckedList("error");
      });
    return () => {
      stale = true;
    };
  }, [showingAcked, props.ackedCount, props.flagsVersion]);

  const ackedLoaded = Array.isArray(ackedList);
  const source = showingAcked ? (ackedLoaded ? ackedList : NO_EVENTS) : props.flagged;
  const rows = useMemo<Row[]>(
    () =>
      source.map((event) => ({
        event,
        category: categoryOf(event.flag ?? ""),
        severity: severityOf(event.flag ?? "", event.summary),
      })),
    [source],
  );

  const counts = useMemo(() => {
    const map = new Map<Category, number>();
    for (const row of rows) map.set(row.category, (map.get(row.category) ?? 0) + 1);
    return map;
  }, [rows]);

  const filtered = useMemo(
    () =>
      rows.filter((row) => {
        if (agent !== "all" && row.event.agent !== agent) return false;
        if (category !== "all" && row.category !== category) return false;
        if (!query) return true;
        const q = query.toLowerCase();
        return (
          (row.event.summary ?? "").toLowerCase().includes(q) ||
          (row.event.flag ?? "").toLowerCase().includes(q) ||
          projectName(row.event.cwd).toLowerCase().includes(q)
        );
      }),
    [rows, agent, category, query],
  );

  // Groups hold every matching row (so "acknowledge all" means all); only
  // the first `limit` rows across the groups are rendered.
  const groups = useMemo(
    () =>
      SEVERITY_LABELS.map((tier) => ({
        ...tier,
        items: filtered.filter((row) => row.severity === tier.severity),
      })).filter((group) => group.items.length > 0),
    [filtered],
  );
  const shownGroups = useMemo(() => {
    let budget = limit;
    return groups.map((group) => {
      const shown = group.items.slice(0, Math.max(0, budget));
      budget -= shown.length;
      return { ...group, shown };
    });
  }, [groups, limit]);
  const shownCount = shownGroups.reduce((n, g) => n + g.shown.length, 0);
  const hidden = filtered.length - shownCount;

  const visibleEvents = useMemo(
    () => shownGroups.flatMap((g) => g.shown.map((row) => row.event)),
    [shownGroups],
  );
  const { onVisibleRows, onOpenEvent } = props;
  useEffect(() => {
    onVisibleRows(visibleEvents, showingAcked);
    return () => onVisibleRows([], false);
  }, [visibleEvents, showingAcked, onVisibleRows]);

  // Stable per-bucket handlers keep the memoized rows from re-rendering.
  const openRow = useCallback(
    (event: AgentEvent) => onOpenEvent(event, showingAcked),
    [onOpenEvent, showingAcked],
  );

  const ackedEmpty = showingAcked && ackedLoaded && rows.length === 0;
  const openEmpty = !showingAcked && rows.length === 0;

  return (
    <main className="view">
      <header className="view-head">
        <h1>Flagged</h1>
        <p className="view-sub">
          An inbox, not a graveyard: acknowledge what you've reviewed. Tracon
          flags; it never blocks.
        </p>
      </header>

      <div className="filterbar">
        <div className="seg">
          <button
            className={showingAcked ? "seg-item" : "seg-item active"}
            onClick={() => setBucket("open")}
          >
            Open {props.flagged.length}
          </button>
          <button
            className={showingAcked ? "seg-item active" : "seg-item"}
            onClick={() => setBucket("acked")}
          >
            Acknowledged {props.ackedCount}
          </button>
        </div>
        <input
          className="search"
          type="search"
          aria-label="Search flagged events"
          placeholder="Search flagged commands, reasons, projects..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <div className="filter-chips">
          {CATEGORY_LABELS.map((c) => {
            const count = c.key === "all" ? rows.length : (counts.get(c.key) ?? 0);
            if (c.key !== "all" && count === 0) return null;
            return (
              <button
                key={c.key}
                className={category === c.key ? "chip active chip-bad" : "chip"}
                onClick={() => setCategory(c.key)}
              >
                {c.label} <span className="chip-count">{count}</span>
              </button>
            );
          })}
        </div>
      </div>

      <AgentChips counts={agentCounts(source)} value={agent} onChange={setAgent} />

      {showingAcked && ackedList === null && <RowsSkeleton />}
      {showingAcked && ackedList === "error" && (
        <p className="list-error">Could not load acknowledged flags. It retries on the next change.</p>
      )}

      {(ackedLoaded || !showingAcked) && filtered.length === 0 ? (
        <div className="pkg-empty">
          {openEmpty ? <Mascot name="inbox-zero" /> : <FlagIcon size={34} />}
          <p>
            {ackedEmpty
              ? "Nothing acknowledged yet."
              : openEmpty
                ? "Inbox zero. Quiet skies."
                : "Nothing matches this filter."}
          </p>
        </div>
      ) : (
        shownGroups.map((group) => (
          <section key={group.severity} className="group">
            <GroupHead
              label={group.label}
              count={group.items.length}
              tone={group.severity}
              action={
                !showingAcked && (
                  <button
                    className="linkish group-action"
                    onClick={() => props.onAckMany(group.items.map((row) => row.event), true)}
                  >
                    acknowledge all
                  </button>
                )
              }
            />
            <ul className="rows" role="listbox" aria-label={`${group.label} flags`}>
              {group.shown.map((row, i) => (
                <EventRow
                  key={row.event.id ?? i}
                  event={row.event}
                  selected={row.event.id !== undefined && row.event.id === props.selectedId}
                  showProject
                  acked={showingAcked}
                  onAck={props.onAck}
                  onOpen={openRow}
                />
              ))}
            </ul>
          </section>
        ))
      )}
      {hidden > 0 && (
        <div className="list-more">
          <button className="thread-btn" onClick={() => setLimit(Number.POSITIVE_INFINITY)}>
            Show {hidden} more
          </button>
        </div>
      )}
      <StatusBar
        left={`${shownCount} of ${filtered.length} ${showingAcked ? "acknowledged" : "open"} flags shown`}
        right={<KeyHints ack={!showingAcked} />}
      />
    </main>
  );
}

function RowsSkeleton() {
  return (
    <ul className="rows" aria-busy="true" aria-label="Loading">
      <li className="row-skeleton" />
      <li className="row-skeleton" />
      <li className="row-skeleton" />
    </ul>
  );
}
