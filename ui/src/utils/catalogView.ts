import type {
  BiblioPublicView,
  BiblioShortPublic,
  BiblioShortStaff,
  BiblioStaffView,
  CatalogNotice,
  HoldDetailsPublic,
  HoldDetailsStaff,
  HoldListItem,
  ItemPublicView,
  ItemStaffView,
  LoanDetailsPublic,
  LoanDetailsStaff,
  LoanListItem,
} from '@/types';

/** Staff notices always include `weedingStatus`. Public notices omit it. */
export function isBiblioStaffView(notice: CatalogNotice): notice is BiblioStaffView {
  return 'weedingStatus' in notice;
}

export function isBiblioPublicView(notice: CatalogNotice): notice is BiblioPublicView {
  return !isBiblioStaffView(notice);
}

/** Staff copies always include `weedingStatus`. Public copies omit it. */
export function isItemStaffView(item: ItemStaffView | ItemPublicView): item is ItemStaffView {
  return 'weedingStatus' in item;
}

export function isBiblioShortStaff(
  biblio: BiblioShortStaff | BiblioShortPublic,
): biblio is BiblioShortStaff {
  return 'weedingStatus' in biblio;
}

export function isLoanDetailsStaff(loan: LoanListItem): loan is LoanDetailsStaff {
  return isBiblioShortStaff(loan.biblio);
}

export function isLoanDetailsPublic(loan: LoanListItem): loan is LoanDetailsPublic {
  return !isLoanDetailsStaff(loan);
}

/**
 * `GET /users/:id/loans` row. Staff receive `LoanDetailsStaff`.
 * A reader opening their own loans receives the minimized borrower (`LoanDetailsPublic`).
 */
export function isUserRecordLoan(loan: LoanListItem): loan is LoanListItem {
  return typeof loan?.id === 'string' && loan.biblio != null;
}

export function isHoldDetailsStaff(hold: HoldListItem): hold is HoldDetailsStaff {
  return isBiblioShortStaff(hold.biblio);
}

export function isHoldDetailsPublic(hold: HoldListItem): hold is HoldDetailsPublic {
  return !isHoldDetailsStaff(hold);
}
