export function fetcher<T>(path: string): T {
  return { path } as unknown as T;
}
