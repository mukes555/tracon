import { useEffect, useState } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { ingestFailureText } from "../lib/captureSources";
import { applyTheme, normalizeTheme, THEME_KEY } from "../lib/theme";
import type { CaptureStatus, ThemeSetting } from "../lib/types";
import { useTransientNote } from "../lib/useTransientNote";
import type { UpdateStatus } from "../lib/types";
import { UpdatesCard } from "./UpdatesCard";
import { ConfirmButton } from "./ConfirmButton";

const THEMES: { value: ThemeSetting; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
];

export function SettingsView(props: {
  paused: boolean;
  onSetPaused: (paused: boolean) => Promise<void>;
  capture: CaptureStatus | null;
  eventCount: number;
  onDeleteAll: () => Promise<void>;
  update: UpdateStatus | null;
  onSetUpdateCheck: (enabled: boolean) => Promise<void>;
  onCheckUpdate: () => Promise<UpdateStatus>;
}) {
  const [theme, setTheme] = useState<ThemeSetting>("dark");
  const [intel, setIntel] = useState<boolean | null>(null);
  const [notify, setNotify] = useState(true);
  const [retention, setRetention] = useState("90");
  const [dataDir, setDataDir] = useState("");
  const [version, setVersion] = useState("");
  const { note, show } = useTransientNote();

  useEffect(() => {
    api.getSetting(THEME_KEY).then((v) => setTheme(normalizeTheme(v))).catch(() => {});
    api
      .getSetting("threat_intel_enabled")
      .then((v) => setIntel(v === "true"))
      .catch(() => setIntel(false));
    api
      .getSetting("retention_days")
      .then((v) => v && setRetention(v))
      .catch(() => {});
    api
      .getSetting("notify_flags")
      .then((v) => setNotify(v !== "false"))
      .catch(() => {});
    api.dataDir().then(setDataDir).catch(() => {});
    api.appVersion().then(setVersion).catch(() => {});
  }, []);

  // Every save reports the same way: the success line only once the
  // recorder confirmed, "Could not save" otherwise.
  const save = async (write: Promise<unknown>, doneText: string, revert?: () => void) => {
    try {
      await write;
      show(doneText);
    } catch {
      revert?.();
      show("Could not save", "bad");
    }
  };

  const toggleNotify = () => {
    const next = !notify;
    setNotify(next);
    const text = next ? "Flag notifications on" : "Flag notifications off";
    save(api.setSetting("notify_flags", String(next)), text, () => setNotify(!next));
  };

  const chooseTheme = (value: ThemeSetting) => {
    setTheme(value);
    applyTheme(value);
    save(api.setSetting(THEME_KEY, value), "Theme saved");
  };

  const toggleIntel = () => {
    const next = !intel;
    setIntel(next);
    const text = next ? "Threat intelligence on" : "Threat intelligence off";
    save(api.setSetting("threat_intel_enabled", String(next)), text, () => setIntel(!next));
  };

  const saveRetention = () => {
    const days = parseInt(retention, 10);
    if (!Number.isFinite(days) || days < 1) return;
    save(api.setSetting("retention_days", String(days)), `Keeping ${days} days of history`);
  };

  const setPaused = (paused: boolean) => {
    save(props.onSetPaused(paused), paused ? "Capture paused" : "Capture resumed");
  };

  const revealLog = async () => {
    try {
      const path = await api.logPath();
      // Older opener builds lack revealItemInDir; opening the file itself
      // still gets the user to the log.
      await revealItemInDir(path).catch(() => openPath(path));
    } catch {
      show("Could not open the log", "bad");
    }
  };

  const ingestFailure = ingestFailureText(props.capture);

  return (
    <main className="view">
      <header className="view-head">
        <h1>Settings</h1>
        {note && <span className={note.tone === "bad" ? "saved-note bad" : "saved-note"}>{note.text}</span>}
      </header>

      <section className="card">
        <h3>Capture</h3>
        <div className="seg">
          <button
            className={props.paused ? "seg-item" : "seg-item active"}
            onClick={() => setPaused(false)}
          >
            Recording
          </button>
          <button
            className={props.paused ? "seg-item active" : "seg-item"}
            onClick={() => setPaused(true)}
          >
            Paused
          </button>
        </div>
        <p className="muted">
          While paused, hooks and transcript tailing are ignored and nothing is
          written. Agents keep running; Tracon just stops watching.
        </p>
        {ingestFailure && <p className="ingest-error">{ingestFailure}</p>}
      </section>

      <UpdatesCard
        status={props.update}
        onSetEnabled={props.onSetUpdateCheck}
        onCheckNow={props.onCheckUpdate}
        onNote={show}
      />

      <section className="card">
        <h3>Appearance</h3>
        <div className="seg">
          {THEMES.map((t) => (
            <button
              key={t.value}
              className={theme === t.value ? "seg-item active" : "seg-item"}
              onClick={() => chooseTheme(t.value)}
            >
              {t.label}
            </button>
          ))}
        </div>
        <p className="muted">Light is Tracon's native look. System follows your OS.</p>
      </section>

      <section className="card">
        <h3>Notifications</h3>
        <label className="intel-toggle">
          <input type="checkbox" checked={notify} onChange={toggleNotify} />
          <span>Notify me when an agent action gets flagged</span>
        </label>
        <p className="muted">
          A system notification the moment a recursive delete, pipe-to-shell,
          credential access, or risky package lands, even while Tracon sits in
          the tray.
        </p>
      </section>

      <section className="card">
        <h3>Threat intelligence</h3>
        <label className="intel-toggle">
          <input
            type="checkbox"
            checked={intel ?? false}
            onChange={toggleIntel}
            disabled={intel === null}
          />
          <span>
            Check installed packages against public threat data <em>(off by default)</em>
          </span>
        </label>
        <p className="muted">
          When on, package names (and nothing else) are checked against osv.dev for
          known vulnerabilities and registry.npmjs.org for suspiciously fresh
          versions. This is Tracon's only network feature; your audit data never
          leaves this machine.
        </p>
      </section>

      <section className="card">
        <h3>History</h3>
        <div className="inline-field">
          <input
            className="num"
            type="number"
            min={1}
            value={retention}
            onChange={(e) => setRetention(e.target.value)}
          />
          <span>days of events kept, older ones are purged</span>
          <button className="btn-dark" onClick={saveRetention}>
            Save
          </button>
        </div>
        <div className="inline-field" style={{ marginTop: 12 }}>
          <button
            className="btn-dark"
            onClick={() => save(api.importFullHistory(), "Importing full history in the background")}
          >
            Import full history
          </button>
          <span>
            scan ALL Claude and Codex session files, not just the last 3 days
          </span>
        </div>
      </section>

      <section className="card">
        <h3>Data</h3>
        <div className="inline-field">
          <ConfirmButton
            label="Delete everything"
            confirmLabel={`Really delete ${props.eventCount.toLocaleString()} events?`}
            onConfirm={props.onDeleteAll}
          />
          <span>removes every recorded session, event, and flag from this machine</span>
        </div>
      </section>

      <section className="card">
        <h3>About</h3>
        <dl className="about">
          <dt>Version</dt>
          <dd>{version || "..."}</dd>
          <dt>Data location</dt>
          <dd>
            <code>{dataDir || "..."}</code>
          </dd>
          <dt>Log file</dt>
          <dd>
            <button className="linkish" onClick={revealLog}>
              Reveal log
            </button>
          </dd>
          <dt>Ingest endpoint</dt>
          <dd>
            <code>http://127.0.0.1:48620/ingest</code> (localhost only)
          </dd>
          <dt>Privacy</dt>
          <dd>Local-only by default. No telemetry. Tracon never modifies agent configs.</dd>
          <dt>License</dt>
          <dd>AGPL-3.0, free forever for individual use.</dd>
        </dl>
      </section>
    </main>
  );
}
