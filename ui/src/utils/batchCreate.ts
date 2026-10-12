type Translate = (key: string) => string;

export interface BatchCreateItemError {
  code?: string | null;
  error?: string | null;
}

/**
 * User-facing text for one `POST /loans/batch-create` result.
 * A present `code` is translated via `errors.apiCode.*`. An unknown code uses
 * `errors.generic` and does not show the English `error` string. `error` is
 * used only when `code` is missing (older servers).
 */
export function batchCreateItemMessage(item: BatchCreateItemError, t: Translate): string {
  const code = typeof item.code === 'string' ? item.code.trim() : '';
  if (code) {
    const key = `errors.apiCode.${code}`;
    const translated = t(key);
    if (translated && translated !== key) return translated;
    return t('errors.generic');
  }
  const raw = typeof item.error === 'string' ? item.error.trim() : '';
  return raw || t('errors.generic');
}
