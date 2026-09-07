/// The one boolean control in the app. A native checkbox renders in the
/// OS accent color, which fights the palette everywhere it appears.
export function Switch(props: {
  checked: boolean;
  onChange: () => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      role="switch"
      aria-checked={props.checked}
      aria-label={props.label}
      className={props.checked ? "switch on" : "switch"}
      disabled={props.disabled}
      onClick={props.onChange}
    >
      <span className="switch-knob" />
    </button>
  );
}
