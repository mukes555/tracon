import type { Stats, View } from "../lib/types";
import { BoxIcon, CamIcon, FlagIcon, GearIcon, ListIcon, RadarIcon, TraconMark } from "./icons";

const ITEMS: { view: View; label: string; icon: React.ReactNode }[] = [
  { view: "overview", label: "Overview", icon: <RadarIcon /> },
  { view: "live", label: "Live", icon: <CamIcon /> },
  { view: "timeline", label: "Timeline", icon: <ListIcon /> },
  { view: "packages", label: "Packages", icon: <BoxIcon /> },
  { view: "flagged", label: "Flagged", icon: <FlagIcon /> },
  { view: "settings", label: "Settings", icon: <GearIcon /> },
];

export function NavRail(props: {
  view: View;
  stats: Stats | null;
  liveCount: number;
  onNavigate: (v: View) => void;
  onOpenPalette: () => void;
}) {
  const badgeFor = (view: View): number | null => {
    if (view === "live") return props.liveCount;
    if (!props.stats) return null;
    if (view === "packages") return props.stats.package_count;
    if (view === "flagged") return props.stats.flagged_count;
    return null;
  };

  return (
    <nav className="navrail">
      <div className="drag-strip" data-tauri-drag-region />
      <div className="brand" data-tauri-drag-region>
        <span className="brand-mark">
          <TraconMark size={16} />
        </span>
        <span className="brand-name">Tracon</span>
      </div>
      <button className="nav-search" onClick={props.onOpenPalette}>
        <span>Search</span>
        <kbd className="kbd">⌘K</kbd>
      </button>
      <ul>
        {ITEMS.map((item) => {
          const badge = badgeFor(item.view);
          const isAlert = item.view === "flagged" && (badge ?? 0) > 0;
          return (
            <li key={item.view}>
              <button
                className={`nav-item${props.view === item.view ? " active" : ""}${isAlert ? " alert" : ""}`}
                onClick={() => props.onNavigate(item.view)}
              >
                {item.icon}
                <span>{item.label}</span>
                {badge !== null && badge > 0 && <span className="nav-badge">{badge}</span>}
              </button>
            </li>
          );
        })}
      </ul>
      <div className="navrail-foot">
        {props.stats ? `${props.stats.event_count.toLocaleString()} events recorded` : "connecting..."}
      </div>
    </nav>
  );
}
