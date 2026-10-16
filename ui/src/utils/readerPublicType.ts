import type { TFunction } from 'i18next';
import type { PublicType } from '@/types';
import { getApiErrorCode } from '@/utils/apiError';
import { localizedPublicTypeById, localizedPublicTypeLabel } from '@/utils/publicTypeDisplay';

/** Account type whose public type is computed from the birth date. */
export const READER_ACCOUNT_TYPE = 'reader';

export const BIRTH_DATE_REQUIRED = 'birth_date_required';
export const NO_PUBLIC_TYPE_FOR_AGE = 'no_public_type_for_age';

/** Open-ended bands sort after every closed span. Matches the server rank. */
const OPEN_BAND_WIDTH = 1_000_000_000;

type AgeBand = Pick<PublicType, 'id' | 'name' | 'ageMin' | 'ageMax'>;

export function isReaderAccount(accountType: string | null | undefined): boolean {
  return (accountType ?? '').trim().toLowerCase() === READER_ACCOUNT_TYPE;
}

/**
 * Birth date is required only when creating a reader.
 * An existing reader may be saved without one: the stored public type is kept.
 * Staff and other account types may omit it. Their public type is not computed.
 */
export function readerBirthDateRequired(
  mode: 'create' | 'edit',
  accountType: string | null | undefined,
): boolean {
  return mode === 'create' && isReaderAccount(accountType);
}

/** Completed years on `on` (UTC calendar day). The anniversary counts as the new age. */
export function completedAgeYears(birthdate: string, on: Date = new Date()): number | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(birthdate.trim());
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  if (month < 1 || month > 12 || day < 1 || day > 31) return null;
  const onYear = on.getUTCFullYear();
  const onMonth = on.getUTCMonth() + 1;
  const onDay = on.getUTCDate();
  let years = onYear - year;
  if (onMonth < month || (onMonth === month && onDay < day)) years -= 1;
  return years;
}

function isAgeBand(band: AgeBand): boolean {
  return band.ageMin != null || band.ageMax != null;
}

function bandWidth(band: AgeBand): number {
  if (band.ageMin != null && band.ageMax != null) return band.ageMax - band.ageMin;
  return OPEN_BAND_WIDTH;
}

function compareId(left: string, right: string): number {
  if (/^\d+$/.test(left) && /^\d+$/.test(right)) {
    const a = BigInt(left);
    const b = BigInt(right);
    if (a < b) return -1;
    if (a > b) return 1;
    return 0;
  }
  return left < right ? -1 : left > right ? 1 : 0;
}

/**
 * Public-type id for a completed age.
 * Bands with no bounds are ignored. Narrowest span wins, then the lowest id.
 * No match is null — the server answers 422 `no_public_type_for_age`.
 */
export function resolvePublicTypeIdForAge(
  bands: readonly AgeBand[],
  age: number,
): string | null {
  const matches = bands.filter((band) => {
    if (!isAgeBand(band)) return false;
    if (band.ageMin != null && age < band.ageMin) return false;
    if (band.ageMax != null && age > band.ageMax) return false;
    return true;
  });
  matches.sort((left, right) => {
    const width = bandWidth(left) - bandWidth(right);
    if (width !== 0) return width;
    return compareId(String(left.id), String(right.id));
  });
  return matches[0] ? String(matches[0].id) : null;
}

/**
 * Public type that decides the guardian field.
 * A reader with a birth date uses the age band. No band, or no birth date, keeps the stored id.
 * The read-only caption still comes from the server response, not from this preview.
 */
export function guardianPublicTypeId(input: {
  accountType: string | null | undefined;
  birthdate: string;
  storedPublicTypeId: string;
  publicTypes: readonly AgeBand[];
  today?: Date;
}): string {
  if (!isReaderAccount(input.accountType)) return input.storedPublicTypeId;
  const birthdate = input.birthdate.trim();
  if (!birthdate) return input.storedPublicTypeId;
  const age = completedAgeYears(birthdate, input.today);
  if (age == null || age < 0) return input.storedPublicTypeId;
  return resolvePublicTypeIdForAge(input.publicTypes, age) ?? input.storedPublicTypeId;
}

/** Caption for a staff id or an `/auth/me` public-type name. */
export function publicTypeCaption(
  t: (key: string) => string,
  publicTypes: readonly PublicType[],
  value: string | number | null | undefined,
): string | null {
  if (value == null || String(value).trim() === '') return null;
  const raw = String(value).trim();
  if (/^\d+$/.test(raw)) return localizedPublicTypeById(t, publicTypes, raw);
  const match = publicTypes.find((row) => row.name.toLowerCase() === raw.toLowerCase());
  const label = localizedPublicTypeLabel(t, match ?? { name: raw, label: raw });
  return label === '' ? null : label;
}

/** Field message for the two reader birth-date 422 codes. Null for every other error. */
export function readerBirthDateFieldError(error: unknown, t: TFunction): string | null {
  const code = getApiErrorCode(error);
  if (code === BIRTH_DATE_REQUIRED) return t('errors.apiCode.birth_date_required');
  if (code === NO_PUBLIC_TYPE_FOR_AGE) return t('errors.apiCode.no_public_type_for_age');
  return null;
}
