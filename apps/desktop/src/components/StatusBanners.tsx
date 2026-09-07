import type { Connection } from "../lib/useAppData";

/// The strip under the top bar that says when what you see is not the
/// whole story: the recorder is unreachable, or capture is switched off.
export function StatusBanners(props: {
  connection: Connection;
  paused: boolean;
  onResume: () => void;
}) {
  return (
    <>
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
    </>
  );
}
