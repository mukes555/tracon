import { useState } from "react";
import type { CaptureStatus } from "../lib/types";
import { CAPTURE_SOURCES, type CaptureSource, sourceEventCount } from "../lib/captureSources";
import { useTransientNote } from "../lib/useTransientNote";

/// Where events come from and how to connect what is not: one line per
/// source, expanding into setup instructions for the ones that need them.
export function CaptureCard(props: { capture: CaptureStatus | null }) {
  const [openKey, setOpenKey] = useState<string | null>(null);
  // The note text is the key of the snippet that was copied, so only that
  // row's button flips to "copied".
  const copied = useTransientNote();
  if (!props.capture) return null;
  const capture = props.capture;

  const copy = async (key: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      copied.show(key);
    } catch {
      // Clipboard unavailable; the snippet is still visible to copy by hand.
    }
  };

  return (
    <section className="card">
      <h3>Capture sources</h3>
      <ul className="setup-list">
        {CAPTURE_SOURCES.map((s) => {
          const count = sourceEventCount(capture, s);
          const live = count > 0;
          const expandable = !live && !s.auto;
          return (
            <li key={s.key}>
              <button
                className="setup-row"
                onClick={() => expandable && setOpenKey(openKey === s.key ? null : s.key)}
                disabled={!expandable}
              >
                <span className={live ? "dot ok" : "dot"} />
                <span className="capture-label">{s.label}</span>
                <span className="capture-detail">
                  {live ? `live · ${count} events` : idleText(capture, s)}
                </span>
              </button>
              {openKey === s.key && s.how && (
                <div className="setup-detail">
                  <p className="muted">{s.how}</p>
                  {s.snippet && (
                    <div className="snippet-row">
                      <code className="snippet">{s.snippet}</code>
                      <button className="chip" onClick={() => copy(s.key, s.snippet ?? "")}>
                        {copied.note?.text === s.key ? "copied" : "copy"}
                      </button>
                    </div>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
}

function idleText(capture: CaptureStatus, s: CaptureSource): string {
  if (s.auto) return s.auto;
  const notInstalled = s.installed !== undefined && !s.installed(capture);
  if (notInstalled && s.missing) return `${s.missing} · click to set up`;
  return "not connected · click to set up";
}
