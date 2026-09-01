import type { ThemeSetting } from "./types";

export const THEME_KEY = "theme";

/** Light is the identity default; "system" follows the OS via CSS. */
export function applyTheme(setting: ThemeSetting) {
  const root = document.documentElement;
  if (setting === "system") {
    root.removeAttribute("data-theme");
  } else {
    root.setAttribute("data-theme", setting);
  }
}

export function normalizeTheme(value: string | null): ThemeSetting {
  if (value === "dark" || value === "system") return value;
  return "light";
}
