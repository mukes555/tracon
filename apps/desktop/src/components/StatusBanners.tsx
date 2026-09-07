import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Connection } from "../lib/useAppData";
import type { UpdateStatus } from "../lib/types";

/// The strip under the top bar that says when what you see is not the
/// whole story: the recorder is unreachable, or capture is switched off.
export function StatusBanners(props: {
  connection: Connection;
  paused: boolean;
  onResume: () => void;
  update: UpdateStatus | null;
  ingestFailure: string | null;
}) {
  return (
    <>
      {props.ingestFailure && (
        <p className="banner bad" role="alert">
          {props.ingestFailure}
        </p>
      )}
      {props.connection === "lost" && (
        <p className="banner lost" role="status">
          Lost contact with the recorder, retrying
        </p>
      )}
      {props.paused && (
        <div className="banner warn" role="status">
          <span>Capture is paused. Nothing is being recorded.</span>
          <button className="banner-btn" onClick={props.onResume}>
            Resume
          </button>
        </div>
      )}
      {props.update?.available && <UpdateBanner status={props.update} />}
    </>
  );
}

const BREW_UPGRADE = "brew upgrade --cask tracon";

/// Homebrew installs upgrade with one command, so the banner hands it over;
/// everything else goes to the release page.
function UpdateBanner(props: { status: UpdateStatus }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(BREW_UPGRADE);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2500);
    } catch {
      // Clipboard unavailable; the command is visible to copy by hand.
    }
  };
  const open = () => {
    openUrl(props.status.url ?? "https://github.com/mukes555/tracon/releases/latest").catch(() => {});
  };
  return (
    <div className="banner info" role="status">
      <span>
        Tracon {props.status.latest} is available.
        {props.status.via_brew && (
          <>
            {" "}
            Run <code>{BREW_UPGRADE}</code>
          </>
        )}
      </span>
      {props.status.via_brew ? (
        <button className="banner-btn" onClick={copy}>
          {copied ? "Copied" : "Copy command"}
        </button>
      ) : (
        <button className="banner-btn" onClick={open}>
          Download
        </button>
      )}
    </div>
  );
}
