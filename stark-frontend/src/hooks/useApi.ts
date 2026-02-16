import { useState, useEffect, useCallback } from 'react';
import { apiFetch } from '@/lib/api';

interface UseApiOptions {
  immediate?: boolean;
}

interface UseApiReturn<T> {
  data: T | null;
  error: string | null;
  isLoading: boolean;
  refetch: () => Promise<void>;
}

export function useApi<T>(
  endpoint: string,
  options: UseApiOptions = { immediate: true }
): UseApiReturn<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(options.immediate ?? true);

  const fetchData = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const result = await apiFetch<T>(endpoint);
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setIsLoading(false);
    }
  }, [endpoint]);

  useEffect(() => {
    if (options.immediate) {
      fetchData();
    }
  }, [fetchData, options.immediate]);

  return {
    data,
    error,
    isLoading,
    refetch: fetchData,
  };
}

interface UseMutationReturn<TData, TParams> {
  data: TData | null;
  error: string | null;
  isLoading: boolean;
  mutate: (params: TParams) => Promise<TData | null>;
  reset: () => void;
}

export function useMutation<TData, TParams>(
  mutationFn: (params: TParams) => Promise<TData>
): UseMutationReturn<TData, TParams> {
  const [data, setData] = useState<TData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const mutate = useCallback(
    async (params: TParams): Promise<TData | null> => {
      setIsLoading(true);
      setError(null);

      try {
        const result = await mutationFn(params);
        setData(result);
        return result;
      } catch (err) {
        const errorMessage = err instanceof Error ? err.message : 'An error occurred';
        setError(errorMessage);
        return null;
      } finally {
        setIsLoading(false);
      }
    },
    [mutationFn]
  );

  const reset = useCallback(() => {
    setData(null);
    setError(null);
    setIsLoading(false);
  }, []);

  return {
    data,
    error,
    isLoading,
    mutate,
    reset,
  };
}
