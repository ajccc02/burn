"use client";
import { useCallback, useEffect, useRef, useState } from "react";
import type { Result } from "@/lib/types";
export function useResource<T>(loader: () => Promise<Result<T>>) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const sequence = useRef(0);
  const reload = useCallback(async () => {
    await Promise.resolve();
    const current = ++sequence.current;
    setLoading(true);
    setError(null);
    try {
      const result = await loader();
      if (current !== sequence.current) return;
      if (result.ok) setData(result.data);
      else setError(result.error);
    } catch {
      if (current === sequence.current)
        setError(
          "The request could not be completed. Check your connection and retry.",
        );
    } finally {
      if (current === sequence.current) setLoading(false);
    }
  }, [loader]);
  useEffect(() => {
    let active = true;
    queueMicrotask(() => {
      if (active) void reload();
    });
    return () => {
      active = false;
      sequence.current += 1;
    };
  }, [reload]);
  return { data, error, loading, reload };
}
