import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { labels, type LabelKey, type LocaleDictionary } from "@/lib/labels";

export const SUPPORTED_LOCALES = ["zh-CN", "en"] as const;
export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];

export const LOCALE_STORAGE_KEY = "plurora-language";

type LabelValue<K extends LabelKey> = LocaleDictionary[K];
type LabelArgs<K extends LabelKey> = LabelValue<K> extends (...args: infer Args) => string
  ? Args
  : [];

export function normalizeLocale(value: string | null | undefined): SupportedLocale | null {
  if (!value) return null;
  const normalized = value.trim().toLowerCase().replace("_", "-");
  if (normalized === "en" || normalized.startsWith("en-")) return "en";
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh-CN";
  return null;
}

function safeStorage(): Storage | null {
  try {
    if (typeof localStorage !== "undefined") return localStorage;
    if (typeof window !== "undefined") return window.localStorage;
  } catch {
    return null;
  }
  return null;
}

export function chooseInitialLocale({
  saved,
  browser,
  fallback = "zh-CN",
}: {
  saved?: string | null;
  browser?: string | null;
  fallback?: SupportedLocale;
}): SupportedLocale {
  return normalizeLocale(saved) ?? normalizeLocale(browser) ?? fallback;
}

export function readInitialLocale(): SupportedLocale {
  const storage = safeStorage();
  const saved = storage?.getItem(LOCALE_STORAGE_KEY) ?? null;
  const browser = typeof navigator !== "undefined" ? navigator.language : null;
  return chooseInitialLocale({ saved, browser });
}

export function lookupLabel<K extends LabelKey>(
  locale: SupportedLocale,
  key: K,
  ...args: LabelArgs<K>
): string {
  const value = labels[locale][key];
  if (typeof value === "function") {
    return (value as unknown as (...innerArgs: LabelArgs<K>) => string)(...args);
  }
  return value;
}

const LocaleContext = createContext<{
  locale: SupportedLocale;
  setLocale: (locale: SupportedLocale) => void;
  t: <K extends LabelKey>(key: K, ...args: LabelArgs<K>) => string;
}>({
  locale: "zh-CN",
  setLocale: () => {},
  t: (key, ...args) => lookupLabel("zh-CN", key, ...args),
});

export function LocaleProvider({ children }: { children?: ReactNode }) {
  const [locale, setLocaleState] = useState<SupportedLocale>(readInitialLocale);

  useEffect(() => {
    if (typeof document !== "undefined") {
      document.documentElement.lang = locale;
    }
  }, [locale]);

  const setLocale = useCallback((nextLocale: SupportedLocale) => {
    setLocaleState(nextLocale);
    safeStorage()?.setItem(LOCALE_STORAGE_KEY, nextLocale);
  }, []);

  const t = useCallback(
    <K extends LabelKey>(key: K, ...args: LabelArgs<K>) => lookupLabel(locale, key, ...args),
    [locale],
  );

  const value = useMemo(() => ({ locale, setLocale, t }), [locale, setLocale, t]);

  return <LocaleContext.Provider value={value}>{children}</LocaleContext.Provider>;
}

export function useLocale() {
  return useContext(LocaleContext);
}

export function useT() {
  return useLocale().t;
}
