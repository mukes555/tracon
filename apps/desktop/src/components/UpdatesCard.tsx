import { useState } from "react";
import type { UpdateStatus } from "../lib/types";
import { relTime } from "../lib/format";

/// Settings card for the opt-in version check. Off by default: nothing
/// touches the network until the user says so, and a manual check is
/// always available.
export function UpdatesCard(props: {
  status: UpdateStatus | null;
  onSetEnabled: (enabled: boolean) => Promise<void>;
  onCheckNow: () => Promise<UpdateStatus>;
  onNote: (text: string, tone?: "bad") => void;
}) {
  const [checking, setChecking] = useState(false);
  const s = props.status;
  const enabled = s?.enabled ?? false;

  const toggle = async (on: boolean) => {
    try {
      await props.onSetEnabled(on);
      props.onNote(on ? "Daily update check on" : "Daily update check off");
    } catch {
      props.onNote("Could not save", "bad");
    }
  };

  const check = async () => {
    setChecking(true);
    try {
      const next = await props.onCheckNow();
      props.onNote(next.available ? `Tracon ${next.latest} is available` : "You are on the latest version");
    } catch {
      props.onNote("Could not reach GitHub", "bad");
    } finally {
      setChecking(false);
    }
  };

  return (
    <section className="card">
      <h3>Updates</h3>
      <div className="seg">
        <button className={enabled ? "seg-item" : "seg-item active"} onClick={() => toggle(false)}>
          Manual
        </button>
        <button className={enabled ? "seg-item active" : "seg-item"} onClick={() => toggle(true)}>
          Daily check
        </button>
      </div>
      <p className="muted">
        The daily check asks GitHub for the latest release number and nothing
        else; no data about you or your machine is sent. Off by default.
      </p>
      <div className="update-row">
        <button className="ack-btn" onClick={check} disabled={checking}>
          {checking ? "Checking" : "Check now"}
        </button>
        <span className="muted">
          {s?.checked_at ? `Last checked ${relTime(s.checked_at)}` : "Never checked"}
          {s?.available && s.latest ? ` · ${s.latest} available` : ""}
        </span>
      </div>
    </section>
  );
}
