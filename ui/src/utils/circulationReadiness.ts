/** Mirrors server `Item::ready_to_circulate`: barcode + site + (price or deferral). */

export type CirculationReadinessInput = {
  barcode?: string | null;
  sourceId?: string | null;
  sourceName?: string | null;
  price?: string | null;
  priceDeferred?: boolean | null;
};

export type CirculationField = 'barcode' | 'site' | 'price';

export function nonemptyText(value?: string | null): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

export function missingCirculationFields(input: CirculationReadinessInput): CirculationField[] {
  const missing: CirculationField[] = [];
  if (nonemptyText(input.barcode) == null) missing.push('barcode');
  if (nonemptyText(input.sourceId) == null && nonemptyText(input.sourceName) == null) {
    missing.push('site');
  }
  if (!input.priceDeferred && nonemptyText(input.price) == null) missing.push('price');
  return missing;
}

export function isReadyToCirculate(input: CirculationReadinessInput): boolean {
  return missingCirculationFields(input).length === 0;
}

export function completeItemHref(biblioId: string, itemId: string): string {
  return `/biblios/${biblioId}?completeItem=${encodeURIComponent(itemId)}`;
}

export function isCirculationReadinessError(message: string): boolean {
  return /cannot be made borrowable|price deferral|barcode, site/i.test(message);
}
