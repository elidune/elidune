import type { TFunction } from 'i18next';
import type { Fine, FineChargeType, FineStatus } from '@/types';

const CHARGE_TYPES = new Set<FineChargeType>(['overdue', 'replacement', 'damage']);
const STATUSES = new Set<FineStatus>([
  'pending',
  'partial',
  'paid',
  'waived',
  'cancelled',
  'refunded',
]);

/**
 * `GET /users/{id}/fines` requires users-read.
 * Without that right the staff record must not call it.
 */
export function mayLoadPatronFines(canReadUsersRight: boolean): boolean {
  return canReadUsersRight;
}

/** Decimal euro amount from the fines API (`"12.50"` or a JSON number). */
export function parseEuroAmount(value: string | number | null | undefined): number {
  if (typeof value === 'number') return Number.isFinite(value) ? value : 0;
  if (typeof value !== 'string') return 0;
  const trimmed = value.trim();
  if (!trimmed) return 0;
  const amount = Number(trimmed);
  return Number.isFinite(amount) ? amount : 0;
}

/** `12,50 €` in French, `€12.50` in English. */
export function formatEuroAmount(value: string | number | null | undefined, locale: string): string {
  return new Intl.NumberFormat(locale, { style: 'currency', currency: 'EUR' }).format(
    parseEuroAmount(value),
  );
}

export function fineReasonLabel(
  chargeType: string | null | undefined,
  t: TFunction,
): string {
  if (chargeType && CHARGE_TYPES.has(chargeType as FineChargeType)) {
    return t(`users.fines.reasons.${chargeType}`);
  }
  return t('users.fines.reasons.unknown');
}

export function fineStatusLabel(status: string | null | undefined, t: TFunction): string {
  if (status && STATUSES.has(status as FineStatus)) {
    return t(`users.fines.status.${status}`);
  }
  return status?.trim() || t('users.fines.reasons.unknown');
}

/** Notes are server text. Shown only when they add something beyond the charge type. */
export function fineNote(fine: Pick<Fine, 'notes'>): string | null {
  const notes = fine.notes?.trim();
  return notes ? notes : null;
}
