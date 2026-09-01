import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { applyTheme, normalizeTheme, THEME_KEY } from "../lib/theme";
import type { ThemeSetting } from "../lib/types";

const THEMES: { value: ThemeSetting; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
];

export function SettingsView() {
  const [theme, setTheme] = useState<ThemeSetting>("dark");
  const [intel, setIntel] = useState<boolean | null>(null);
  const [notify, setNotify] = useState(true);
  const [retention, setRetention] = useState("90");
  const [dataDir, setDataDir] = useState("");
  const [saved, setSaved] = useState<string | null>(null);

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
  }, []);

  const toggleNotify = async () => {
    const next = !notify;
    setNotify(next);
    await api.setSetting("notify_flags", next ? "true" : "false").catch(() => setNotify(!next));
    note(next ? "Flag notifications on" : "Flag notifications off");
  };

  const note = (text: string) => {
    setSaved(text);
    setTimeout(() => setSaved(null), 2500);
  };

  const chooseTheme = async (value: ThemeSetting) => {
    setTheme(value);
    applyTheme(value);
    await api.setSetting(THEME_KEY, value).catch(() => {});
    note("Theme saved");
  };

  const toggleIntel = async () => {
    const next = !intel;
    setIntel(next);
    await api.setSetting("threat_intel_enabled", next ? "true" : "false").catch(() => setIntel(!next));
    note(next ? "Threat intelligence on" : "Threat intelligence off");
  };

  const saveRetention = async () => {
    const days = parseInt(retention, 10);
    if (!Number.isFinite(days) || days < 1) return;
    await api.setSetting("retention_days", String(days)).catch(() => {});
    note(`Keeping ${days} days of history`);
  };

  return (
    <main className="view settings">
      <header className="view-head">
        <h1>Settings</h1>
        {saved && <span className="saved-note">{saved}</span>}
      </header>

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
            onClick={async () => {
              await api.importFullHistory().catch(() => {});
              note("Importing full history in the background");
            }}
          >
            Import full history
          </button>
          <span>
            scan ALL Claude and Codex session files, not just the last 3 days
          </span>
        </div>
      </section>

      <section className="card">
        <h3>About</h3>
        <dl className="about">
          <dt>Data location</dt>
          <dd>
            <code>{dataDir || "..."}</code>
          </dd>
          <dt>Ingest endpoint</dt>
          <dd>
            <code>http://localhost:48620/ingest</code> (localhost only)
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
