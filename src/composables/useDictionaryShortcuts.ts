import { reactive } from "vue";

export interface DictionaryShortcutSettings {
  dictionaryKey: string;
  choiceKey: string;
}

export const dictionaryShortcutKeyOptions = [
  { value: "", label: "关闭" },
  { value: "d", label: "D" },
  { value: "f", label: "F" },
  { value: "j", label: "J" },
  { value: "k", label: "K" },
  { value: "[", label: "[" },
  { value: "]", label: "]" },
  { value: ";", label: ";" },
] as const;

const storageKey = "kotoclip.dictionary-shortcuts.v1";
const defaults: DictionaryShortcutSettings = {
  dictionaryKey: "d",
  choiceKey: "f",
};

function loadSettings(): DictionaryShortcutSettings {
  if (typeof window === "undefined") return { ...defaults };
  try {
    const saved = JSON.parse(window.localStorage.getItem(storageKey) ?? "{}") as Partial<DictionaryShortcutSettings>;
    return {
      dictionaryKey: typeof saved.dictionaryKey === "string" ? saved.dictionaryKey : defaults.dictionaryKey,
      choiceKey: typeof saved.choiceKey === "string" ? saved.choiceKey : defaults.choiceKey,
    };
  } catch {
    return { ...defaults };
  }
}

export const dictionaryShortcutSettings = reactive(loadSettings());

function saveSettings() {
  if (typeof window === "undefined") return;
  window.localStorage.setItem(storageKey, JSON.stringify(dictionaryShortcutSettings));
}

export function setDictionaryShortcut(name: keyof DictionaryShortcutSettings, key: string) {
  dictionaryShortcutSettings[name] = key;
  saveSettings();
}

export function shortcutKeyLabel(key: string) {
  return dictionaryShortcutKeyOptions.find((option) => option.value === key)?.label ?? key.toUpperCase();
}

export function matchesDictionaryShortcut(event: KeyboardEvent, key: string, shift = false) {
  return Boolean(
    key
    && event.key.toLocaleLowerCase() === key.toLocaleLowerCase()
    && event.shiftKey === shift
    && !event.ctrlKey
    && !event.altKey
    && !event.metaKey
  );
}
