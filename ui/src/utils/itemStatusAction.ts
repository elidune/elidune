type Translate = (key: string) => string;

/** `itemStatus` values on `item_status_action_required`. */
const ITEM_STATUS_ACTION_KEY: Record<string, string> = {
  lost: 'errors.itemStatusAction.lost',
  claimedReturned: 'errors.itemStatusAction.claimedReturned',
  toSearch: 'errors.itemStatusAction.toSearch',
  missing: 'errors.itemStatusAction.missing',
};

/**
 * What the agent must do before checkout.
 * An unknown or missing `itemStatus` uses the generic sentence.
 */
export function itemStatusActionMessage(
  itemStatus: string | null | undefined,
  t: Translate,
): string {
  const key = ITEM_STATUS_ACTION_KEY[itemStatus?.trim() ?? ''];
  if (!key) return t('errors.itemStatusAction.generic');
  const text = t(key);
  if (!text || text === key) return t('errors.itemStatusAction.generic');
  return text;
}

/** `itemStatus` on a checkout 422, or null when the field is absent. */
export function readCheckoutItemStatus(error: unknown): string | null {
  if (!error || typeof error !== 'object') return null;
  const response = (error as { response?: unknown }).response;
  if (!response || typeof response !== 'object') return null;
  const data = (response as { data?: unknown }).data;
  if (!data || typeof data !== 'object' || Array.isArray(data)) return null;
  const value = (data as { itemStatus?: unknown }).itemStatus;
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  return trimmed || null;
}
