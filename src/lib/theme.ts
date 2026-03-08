export type ThemeMode = "dark" | "light";

export const THEME_STORAGE_KEY = "lokal-skriveflate-theme";

export function resolveStoredTheme(value: string | null): ThemeMode {
  return value === "light" || value === "dark" ? value : "dark";
}

export function getInitialTheme(): ThemeMode {
  if (typeof window === "undefined") {
    return "dark";
  }

  return resolveStoredTheme(window.localStorage.getItem(THEME_STORAGE_KEY));
}

export function applyTheme(themeMode: ThemeMode) {
  if (typeof document === "undefined") {
    return;
  }

  document.documentElement.dataset.theme = themeMode;
  document.documentElement.style.colorScheme = themeMode;
}
