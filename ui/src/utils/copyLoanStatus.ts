import type { TFunction } from 'i18next';
import type { BorrowerRef } from '@/utils/readerIdentifier';

export interface CopyCirculationFields {
  borrowed?: boolean | null;
  availability?: string | null;
  dueDate?: string | null;
  /**
   * Library-local `YYYY-MM-DD` on a public copy when `availability` is `onLoan`
   * and the loan is not overdue. Absent otherwise, including when overdue.
   * Staff views ignore it. The public UI does not derive overdue from `dueDate`.
   */
  expectedReturnDate?: string | null;
  /** Copy is assigned to a hold, or in transit for one. */
  onHold?: boolean | null;
  loanId?: string | null;
  borrower?: BorrowerRef | null;
}

export function isCopyBorrowed(copy: CopyCirculationFields): boolean {
  return copy.borrowed === true || copy.availability === 'onLoan';
}

export function isCopyOnHold(copy: CopyCirculationFields): boolean {
  return copy.availability === 'onHold';
}

/** Card, initials, or name from the item payload. An id alone is not an identifier. */
export function itemBorrowerIdentity(
  borrower: BorrowerRef | null | undefined,
): BorrowerRef | null {
  if (!borrower) return null;
  const card = borrower.cardNumber?.trim() || borrower.barcode?.trim() || null;
  const initials = borrower.initials?.trim() || null;
  const firstname = borrower.firstname?.trim() || null;
  const lastname = borrower.lastname?.trim() || null;
  if (!card && !initials && !firstname && !lastname) return null;
  return {
    id: borrower.id ?? null,
    cardNumber: borrower.cardNumber ?? null,
    initials: borrower.initials ?? null,
    firstname,
    lastname,
    barcode: borrower.barcode ?? null,
    login: borrower.login ?? null,
  };
}

export function formatCopyDueDate(iso: string | null | undefined, locale: string): string | null {
  if (!iso?.trim()) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleDateString(locale, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
  });
}

const CALENDAR_DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/**
 * `YYYY-MM-DD` as a local calendar date. `new Date('YYYY-MM-DD')` is UTC midnight
 * and shifts the day west of UTC, so the fields are passed to the local constructor.
 */
export function formatLocalCalendarDate(
  iso: string | null | undefined,
  locale: string,
): string | null {
  if (!iso?.trim()) return null;
  const match = CALENDAR_DATE.exec(iso.trim());
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const date = new Date(year, month - 1, day);
  if (date.getFullYear() !== year || date.getMonth() !== month - 1 || date.getDate() !== day) {
    return null;
  }
  return date.toLocaleDateString(locale, {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
}

/** Dated sentence. Null when the copy is not on loan or the server omitted the date. */
export function publicExpectedReturnLine(
  t: TFunction,
  copy: Pick<CopyCirculationFields, 'availability' | 'expectedReturnDate'>,
  locale: string,
): string | null {
  if (copy.availability !== 'onLoan') return null;
  const date = formatLocalCalendarDate(copy.expectedReturnDate, locale);
  if (!date) return null;
  return t('opac.expectedReturn', { date });
}

/**
 * Public/OPAC on-loan label. The server omits `expectedReturnDate` when the loan
 * is overdue; this does not read `dueDate` or decide overdue itself.
 * With a date: "On loan, due back …". Without one: "On loan".
 */
export function publicOnLoanLabel(
  t: TFunction,
  copy: Pick<CopyCirculationFields, 'availability' | 'expectedReturnDate' | 'dueDate'>,
  locale: string,
): string | null {
  if (copy.availability !== 'onLoan') return null;
  return publicExpectedReturnLine(t, copy, locale) ?? t('availability.onLoan');
}
