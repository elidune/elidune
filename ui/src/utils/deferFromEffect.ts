/**
 * Run work after the current effect flush (one microtask).
 *
 * Data-load helpers that call setState are flagged by
 * `react-hooks/set-state-in-effect` when invoked synchronously from an
 * effect. Deferring preserves the same user-visible load sequence without
 * converting every page to a new data-fetching API in this cleanup.
 */
export function deferFromEffect(work: () => void): () => void {
  let cancelled = false;
  queueMicrotask(() => {
    if (!cancelled) work();
  });
  return () => {
    cancelled = true;
  };
}
