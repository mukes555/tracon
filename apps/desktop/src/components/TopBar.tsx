import type { View } from "../lib/types";
import { GearIcon } from "./icons";

/// The bar above every view: a search-first command field (opens the
/// palette), the Simple/Advanced mode switch, and settings. Mirrors the
/// header of a native macOS utility rather than a web app toolbar.
export function TopBar(props: {
  advanced: boolean;
  onAdvanced: (on: boolean) => void;
  onOpenPalette: () => void;
  onNavigate: (v: View) => void;
}) {
  return (
    <header className="topbar" data-tauri-drag-region>
      <button className="cmdbar" onClick={props.onOpenPalette}>
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" aria-hidden="true">
          <circle cx="7" cy="7" r="4.6" />
          <path d="M10.6 10.6 L14 14" />
        </svg>
        <span className="cmdbar-hint">Search sessions, commands, flags, or type &gt; for commands</span>
        <kbd className="kbd">⌘K</kbd>
      </button>
      <div className="seg topbar-seg">
        <button
          className={props.advanced ? "seg-item" : "seg-item active"}
          onClick={() => props.onAdvanced(false)}
        >
          Simple
        </button>
        <button
          className={props.advanced ? "seg-item active" : "seg-item"}
          onClick={() => props.onAdvanced(true)}
        >
          Advanced
        </button>
      </div>
      <button className="icon-btn" onClick={() => props.onNavigate("settings")} aria-label="Settings">
        <GearIcon />
      </button>
    </header>
  );
}
