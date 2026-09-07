import { useEffect, useState } from "react";
import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
import { api } from "../lib/api";
import { ingestFailureText } from "../lib/captureSources";
import { relTime } from "../lib/format";
import { applyTheme, normalizeTheme, THEME_KEY } from "../lib/theme";
import type { CaptureStatus, ThemeSetting, UpdateStatus } from "../lib/types";
import { useTransientNote } from "../lib/useTransientNote";
import { ConfirmButton } from "./ConfirmButton";
import { SettingRow } from "./SettingRow";
import { Switch } from "./Switch";

const THEMES: { value: ThemeSetting; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
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
  const [theme, setTheme] = useState<ThemeSetting>("light");
  const [intel, setIntel] = useState<boolean | null>(null);
  const [notify, setNotify] = useState(true);
  const [retention, setRetention] = useState("90");
  const [dataDir, setDataDir] = useState("");
  const [version, setVersion] = useState("");
  const [checking, setChecking] = useState(false);
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
    if (!Number.isFinite(days) || days < 1) {
      show("Enter a number of days", "bad");
      return;
    }
    save(api.setSetting("retention_days", String(days)), `Keeping ${days} days of history`);
  };

  const toggleUpdateCheck = async () => {
    const next = !(props.update?.enabled ?? false);
    try {
      await props.onSetUpdateCheck(next);
      show(next ? "Daily update check on" : "Daily update check off");
    } catch {
      show("Could not save", "bad");
    }
  };

  const checkUpdate = async () => {
    setChecking(true);
    try {
      const next = await props.onCheckUpdate();
      show(next.available ? `Tracon ${next.latest} is available` : "You are on the latest version");
    } catch {
      show("Could not reach GitHub", "bad");
    } finally {
      setChecking(false);
    }
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
  const update = props.update;

  return (
    <main className="view settings">
      <header className="view-head">
        <h1>Settings</h1>
        {note && (
          <span className={note.tone === "bad" ? "saved-note bad" : "saved-note"}>{note.text}</span>
        )}
      </header>

      <section className="card">
        <h3>Capture</h3>
        <SettingRow
          label="Recording"
          hint="While paused, hooks and transcript tailing are ignored and nothing is written. Agents keep running."
          control={
            <Switch
              checked={!props.paused}
              label="Recording"
              onChange={() =>
                save(
                  props.onSetPaused(!props.paused),
                  props.paused ? "Capture resumed" : "Capture paused",
                )
              }
            />
          }
        />
        <SettingRow
          label="Flag notifications"
          hint="A system notification the moment a recursive delete, pipe to shell, credential access, or risky package lands."
          control={<Switch checked={notify} label="Flag notifications" onChange={toggleNotify} />}
        />
        <SettingRow
          label="Threat intelligence"
          hint="Checks package names, and nothing else, against osv.dev and registry.npmjs.org. Off by default."
          control={
            <Switch
              checked={intel ?? false}
              label="Threat intelligence"
              disabled={intel === null}
              onChange={toggleIntel}
            />
          }
        />
        {ingestFailure && <p className="ingest-error">{ingestFailure}</p>}
      </section>

      <section className="card">
        <h3>Appearance</h3>
        <SettingRow
          label="Theme"
          hint="Light is Tracon's native look. System follows your OS."
          control={
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
          }
        />
      </section>

      <section className="card">
        <h3>Updates</h3>
        <SettingRow
          label="Daily check"
          hint="Asks GitHub for the latest release number and nothing else. No data about you or your machine is sent."
          control={
            <Switch
              checked={update?.enabled ?? false}
              label="Daily update check"
              onChange={toggleUpdateCheck}
            />
          }
        />
        <SettingRow
          label="Version"
          hint={
            update?.checked_at
              ? `Last checked ${relTime(update.checked_at)}`
              : "Never checked for updates"
          }
          control={
            <div className="control-pair">
              <span className="muted">
                {version || "..."}
                {update?.available && update.latest ? ` · ${update.latest} available` : ""}
              </span>
              <button className="btn-quiet" onClick={checkUpdate} disabled={checking}>
                {checking ? "Checking" : "Check now"}
              </button>
            </div>
          }
        />
      </section>

      <section className="card">
        <h3>Data</h3>
        <SettingRow
          label="History"
          hint="Older events are purged automatically."
          control={
            <div className="control-pair">
              <input
                className="num"
                type="number"
                min={1}
                aria-label="Days of history to keep"
                value={retention}
                onChange={(e) => setRetention(e.target.value)}
              />
              <span className="muted">days</span>
              <button className="btn-quiet" onClick={saveRetention}>
                Save
              </button>
            </div>
          }
        />
        <SettingRow
          label="Import full history"
          hint="Scans every Claude and Codex session file, not just the last 3 days."
          control={
            <button
              className="btn-quiet"
              onClick={() =>
                save(api.importFullHistory(), "Importing full history in the background")
              }
            >
              Import
            </button>
          }
        />
        <SettingRow
          label="Delete everything"
          hint="Removes every recorded session, event, and flag from this machine."
          control={
            <ConfirmButton
              label="Delete"
              confirmLabel={`Really delete ${props.eventCount.toLocaleString()} events?`}
              onConfirm={props.onDeleteAll}
            />
          }
        />
      </section>

      <section className="card">
        <h3>About</h3>
        <dl className="about">
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
