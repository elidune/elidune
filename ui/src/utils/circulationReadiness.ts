/** Mirrors server `Item::ready_to_circulate`: barcode + site. */

export type CirculationReadinessInput = {
  barcode?: string | null;
  siteId?: string | null;
  siteName?: string | null;
  price?: string | null;
  priceDeferred?: boolean | null;
};

export type CirculationField = 'barcode' | 'site';
export type RecommendedCirculationField = 'price';

export function nonemptyText(value?: string | null): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

export function missingCirculationFields(input: CirculationReadinessInput): CirculationField[] {
  const missing: CirculationField[] = [];
  if (nonemptyText(input.barcode) == null) missing.push('barcode');
  if (nonemptyText(input.siteId) == null && nonemptyText(input.siteName) == null) {
    missing.push('site');
  }
  return missing;
}

/** Staff hint when a copy can circulate but price is still unknown. */
export function recommendedCirculationFields(input: CirculationReadinessInput): RecommendedCirculationField[] {
  if (input.priceDeferred) return [];
  if (nonemptyText(input.price) == null) return ['price'];
  return [];
}

export function isReadyToCirculate(input: CirculationReadinessInput): boolean {
  return missingCirculationFields(input).length === 0;
}

export function completeItemHref(biblioId: string, itemId: string): string {
  return `/biblios/${biblioId}?completeItem=${encodeURIComponent(itemId)}`;
}

export function isCirculationReadinessError(message: string): boolean {
  return /cannot be made borrowable|barcode and site/i.test(message);
}
