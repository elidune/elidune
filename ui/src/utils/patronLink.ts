const UNUSABLE_IDS = new Set(['', 'undefined', 'null', 'NaN']);

/**
 * First usable patron id. Rejects missing values and the string "undefined"
 * so a link never points at `/users/undefined`.
 */
export function patronId(
  ...candidates: Array<string | number | null | undefined>
): string | null {
  for (const raw of candidates) {
    if (raw == null) continue;
    const id = String(raw).trim();
    if (!UNUSABLE_IDS.has(id)) return id;
  }
  return null;
}
