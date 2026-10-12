import type { AccountTypeDefinition } from '@/types';
import { translateStatLabel } from './codeLabels.ts';

/**
 * Label for an account-type code on desk, patron, and item screens.
 * Built-in keys (`reader`, `librarian`, …) and the English seed names go through i18n.
 * A library-defined name that is not one of those keys is kept as stored.
 */
export function localizedAccountTypeLabel(
  t: (key: string) => string,
  code: string | null | undefined,
  types?: readonly AccountTypeDefinition[],
): string {
  if (code == null || String(code).trim() === '') return '—';
  const raw = String(code).trim();
  const catalogName = types
    ?.find((row) => row.code.toLowerCase() === raw.toLowerCase())
    ?.name?.trim();

  if (catalogName) {
    const translatedName = translateStatLabel(t, catalogName, 'accountType');
    if (translatedName !== catalogName) return translatedName;
    if (catalogName.toLowerCase() !== raw.toLowerCase()) return catalogName;
  }

  const translatedCode = translateStatLabel(t, raw, 'accountType');
  if (translatedCode !== raw) return translatedCode;
  return catalogName || raw;
}

export function accountTypeDisplayName(
  types: AccountTypeDefinition[],
  code: string | null | undefined
): string {
  if (code == null || String(code).trim() === '') return '—';
  const raw = String(code).trim();
  const lower = raw.toLowerCase();
  const match = types.find((t) => t.code.toLowerCase() === lower);
  return match?.name ?? raw;
}

export function defaultAccountTypeCode(types: AccountTypeDefinition[]): string {
  const reader = types.find((t) => t.code.toLowerCase() === 'reader');
  if (reader) return reader.code;
  return types[0]?.code ?? '';
}
