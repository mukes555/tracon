// Minimal 16px stroke icons, currentColor so they follow the theme.
type IconProps = { size?: number };

function Svg(props: IconProps & { children: React.ReactNode }) {
  const s = props.size ?? 16;
  return (
    <svg
      width={s}
      height={s}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {props.children}
    </svg>
  );
}

/** The Tracon brand mark: the bold T, mirroring app-icon.svg. Drawn in
    currentColor so the tile decides the polarity (forest on bright green). */
export const TraconMark = (p: IconProps) => {
  const s = p.size ?? 16;
  return (
    <svg width={s} height={s} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="2.4" y="2.9" width="11.2" height="3.2" rx="0.9" fill="currentColor" />
      <rect x="6.4" y="2.9" width="3.2" height="10.2" rx="0.9" fill="currentColor" />
    </svg>
  );
};

export const CamIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="1.8" y="4" width="9" height="7.4" rx="1.8" />
    <path d="M10.8 6.8 L14.2 5 V11.5 L10.8 9.7" />
    <circle cx="4.9" cy="6.9" r="0.9" fill="currentColor" stroke="none" />
  </Svg>
);

export const RadarIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="6.2" />
    <circle cx="8" cy="8" r="3" />
    <path d="M8 8 L12.5 3.5" />
    <circle cx="10.5" cy="9.8" r="0.8" fill="currentColor" stroke="none" />
  </Svg>
);

export const ListIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5.5 4h8M5.5 8h8M5.5 12h8" />
    <circle cx="2.5" cy="4" r="0.9" fill="currentColor" stroke="none" />
    <circle cx="2.5" cy="8" r="0.9" fill="currentColor" stroke="none" />
    <circle cx="2.5" cy="12" r="0.9" fill="currentColor" stroke="none" />
  </Svg>
);

export const BoxIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M8 1.8 14 5v6L8 14.2 2 11V5Z" />
    <path d="M2 5l6 3 6-3M8 8v6" />
  </Svg>
);

export const FlagIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M3.5 14.5V2.5" />
    <path d="M3.5 3h8.5l-1.8 2.75L12 8.5H3.5" />
  </Svg>
);

export const GearIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="2.4" />
    <path d="M8 1.8v2M8 12.2v2M1.8 8h2M12.2 8h2M3.6 3.6l1.4 1.4M11 11l1.4 1.4M12.4 3.6 11 5M5 11l-1.4 1.4" />
  </Svg>
);

export const TerminalIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="1.8" y="2.8" width="12.4" height="10.4" rx="1.5" />
    <path d="M4.5 6.5 7 8.5l-2.5 2M8.5 10.5h3" />
  </Svg>
);

export const FileIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 1.8h5.5L13 5.3v8.9H4Z" />
    <path d="M9.5 1.8v3.5H13" />
  </Svg>
);

export const ChatIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M2 3.5h12v7.5H8.5L5 13.8v-2.8H2Z" />
  </Svg>
);

export const DotIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="8" cy="8" r="2.2" fill="currentColor" stroke="none" />
  </Svg>
);

export function kindIcon(kind: string, toolName: string | null) {
  if (kind === "package_install") return <BoxIcon />;
  if (kind === "prompt") return <ChatIcon />;
  if (kind === "tool_call" || kind === "tool_result") {
    if (toolName === "Bash" || toolName === "shell") return <TerminalIcon />;
    if (["Edit", "Write", "MultiEdit", "NotebookEdit", "Read"].includes(toolName ?? "")) {
      return <FileIcon />;
    }
    return <TerminalIcon />;
  }
  return <DotIcon />;
}
