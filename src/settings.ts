export type AppThemeMode = "dark" | "light";

export type RatingShortcut = "0" | "1" | "2" | "3" | "4" | "5";
export type ColorShortcut = "red" | "yellow" | "green" | "blue";
export type ViewShortcut = "grid" | "single";

export interface AppShortcuts {
  view: Record<ViewShortcut, string>;
  ratings: Record<RatingShortcut, string>;
  colors: Record<ColorShortcut, string>;
  ratingDown: string;
  ratingUp: string;
}

export interface AppSettings {
  importWorkerCount: number;
  analysisBatchSize: number;
  shortcuts: AppShortcuts;
}

export const SETTINGS_STORAGE_KEY = "photo-organizer-settings";

export const DEFAULT_APP_SETTINGS: AppSettings = {
  importWorkerCount: 2,
  analysisBatchSize: 4,
  shortcuts: {
    view: {
      grid: "g",
      single: "f",
    },
    ratings: {
      "0": "0",
      "1": "1",
      "2": "2",
      "3": "3",
      "4": "4",
      "5": "5",
    },
    colors: {
      red: "6",
      yellow: "7",
      green: "8",
      blue: "9",
    },
    ratingDown: "[",
    ratingUp: "]",
  },
};

const ratingShortcuts: RatingShortcut[] = ["0", "1", "2", "3", "4", "5"];
const colorShortcuts: ColorShortcut[] = ["red", "yellow", "green", "blue"];
const viewShortcuts: ViewShortcut[] = ["grid", "single"];

export function readAppSettings(): AppSettings {
  if (typeof window === "undefined") return cloneDefaultSettings();
  try {
    const raw = window.localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) return cloneDefaultSettings();
    return normalizeAppSettings(JSON.parse(raw) as Partial<AppSettings>);
  } catch {
    return cloneDefaultSettings();
  }
}

export function persistAppSettings(settings: AppSettings) {
  try {
    window.localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // A restricted webview may disable local storage; in-memory settings still work.
  }
}

export function normalizeAppSettings(input: Partial<AppSettings>): AppSettings {
  const inputShortcuts = (input.shortcuts ?? {}) as Partial<AppShortcuts>;
  const inputViews = (inputShortcuts.view ?? {}) as Partial<Record<ViewShortcut, string>>;
  const inputRatings = (inputShortcuts.ratings ?? {}) as Partial<Record<RatingShortcut, string>>;
  const inputColors = (inputShortcuts.colors ?? {}) as Partial<Record<ColorShortcut, string>>;
  const view = Object.fromEntries(
    viewShortcuts.map((shortcut) => [
      shortcut,
      normalizeShortcut(inputViews[shortcut], shortcut === "grid" ? "g" : "f"),
    ]),
  ) as Record<ViewShortcut, string>;
  const ratings = Object.fromEntries(
    ratingShortcuts.map((rating) => [rating, normalizeShortcut(inputRatings[rating], rating)]),
  ) as Record<RatingShortcut, string>;
  const colors = Object.fromEntries(
    colorShortcuts.map((color) => [color, normalizeShortcut(inputColors[color], "")]),
  ) as Record<ColorShortcut, string>;

  return {
    importWorkerCount: clampInteger(input.importWorkerCount, 1, 2, 2),
    analysisBatchSize: clampInteger(input.analysisBatchSize, 1, 8, 4),
    shortcuts: {
      view,
      ratings,
      colors,
      ratingDown: normalizeShortcut(inputShortcuts.ratingDown, "["),
      ratingUp: normalizeShortcut(inputShortcuts.ratingUp, "]"),
    },
  };
}

function cloneDefaultSettings(): AppSettings {
  return normalizeAppSettings(DEFAULT_APP_SETTINGS);
}

function normalizeShortcut(value: unknown, fallback: string) {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim();
  return trimmed ? trimmed.slice(-1) : fallback;
}

function clampInteger(value: unknown, min: number, max: number, fallback: number) {
  const parsed = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.round(Math.min(max, Math.max(min, parsed)));
}
