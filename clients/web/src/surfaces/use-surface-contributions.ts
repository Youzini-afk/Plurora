import { useCallback, useEffect, useState } from "react";
import { usePlurora } from "@/lib/plurora-client";
import {
  parseShellContributions,
  type ShellContribution,
  type ShellContributionSlot,
} from "./shell-contributions";

export interface SurfaceContributionsResource<TContribution extends ShellContribution = ShellContribution> {
  items: TContribution[];
  loading: boolean;
  error?: Error;
  refresh: () => void;
}

export function useSurfaceContributions<TContribution extends ShellContribution = ShellContribution>(
  slot: ShellContributionSlot,
  locale: string,
): SurfaceContributionsResource<TContribution> {
  const client = usePlurora();
  const [items, setItems] = useState<TContribution[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | undefined>();
  const [refreshToken, setRefreshToken] = useState(0);

  const refresh = useCallback(() => {
    setRefreshToken((token) => token + 1);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(undefined);

    client
      .surfaceContributions(slot)
      .then((records: unknown) => {
        if (cancelled) return;
        setItems(parseShellContributions(records, slot, locale) as TContribution[]);
        setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setItems([]);
        setError(err instanceof Error ? err : new Error(String(err)));
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [client, slot, locale, refreshToken]);

  return { items, loading, error, refresh };
}
