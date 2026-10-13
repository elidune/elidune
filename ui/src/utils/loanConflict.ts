import { loanRefusalMayForce } from './circulationForce.ts';
import {
  otherBorrowerFromPayload,
  otherBorrowerFromPerson,
  type DeskCopySnapshot,
  type DeskScanAlert,
  type OtherBorrowerRef,
} from './deskScanAlert.ts';

/** 409 from POST /loans when the copy is on loan to another reader. */
export const ITEM_ON_LOAN_TO_OTHER_BORROWER = 'item_on_loan_to_other_borrower';

/** 409 from POST /loans when the copy is already on loan to this reader. */
export const ITEM_ALREADY_ON_LOAN_TO_BORROWER = 'item_already_on_loan_to_borrower';

export type CheckoutLoanNotice = DeskScanAlert & { loanId: string | null };

interface ConflictContext {
  patronId: string;
  snapshot: DeskCopySnapshot | null;
  listedLoanId?: string | null;
}

function text(value: unknown): string | null {
  if (typeof value === 'string') {
    const trimmed = value.trim();
    return trimmed || null;
  }
  if (typeof value === 'number' && Number.isSafeInteger(value)) return String(value);
  return null;
}

function responseOf(error: unknown): { status: number | null; data: Record<string, unknown> | null } {
  if (!error || typeof error !== 'object') return { status: null, data: null };
  const response = (error as { response?: unknown }).response;
  if (!response || typeof response !== 'object') return { status: null, data: null };
  const record = response as { status?: unknown; data?: unknown };
  const data = record.data;
  return {
    status: typeof record.status === 'number' ? record.status : null,
    data: data && typeof data === 'object' && !Array.isArray(data) ? (data as Record<string, unknown>) : null,
  };
}

function readConflictBorrower(data: Record<string, unknown> | null): OtherBorrowerRef | null {
  if (!data) return null;
  const details =
    data.details && typeof data.details === 'object' && !Array.isArray(data.details)
      ? (data.details as Record<string, unknown>)
      : null;
  const candidates = [data.borrower, data.user, details?.borrower, details?.user, details];
  for (const candidate of candidates) {
    const ref = otherBorrowerFromPayload(candidate);
    if (ref) return ref;
  }
  return null;
}

function snapshotBorrower(snapshot: DeskCopySnapshot | null): OtherBorrowerRef | null {
  if (!snapshot) return null;
  return otherBorrowerFromPerson({
    userId: snapshot.loanUserId,
    cardNumber: snapshot.loanUserCard,
    initials: snapshot.loanUserInitials,
    firstname: snapshot.loanUserFirstname,
    lastname: snapshot.loanUserLastname,
    fullName: snapshot.loanUserName,
  });
}

function readLoanId(data: Record<string, unknown> | null): string | null {
  if (!data) return null;
  const direct = text(data.loanId) ?? text(data.loan_id);
  if (direct) return direct;
  const details = data.details;
  if (!details || typeof details !== 'object' || Array.isArray(details)) return null;
  const nested = details as Record<string, unknown>;
  return text(nested.loanId) ?? text(nested.loan_id);
}

/**
 * Notice for a 409 checkout of a copy that is already on loan.
 * `POST /loans` body is `{ code, error: "Conflict", message }` with no loan id.
 * The English `message` is not shown. Renewal uses the barcode snapshot or the reader's open loan.
 * The same codes are returned with or without `force`; neither opens a force retry.
 */
export function checkoutNoticeForConflict(error: unknown, context: ConflictContext): CheckoutLoanNotice | null {
  const { status, data } = responseOf(error);
  if (status !== 409) return null;
  const code = text(data?.code);
  if (code === ITEM_ON_LOAN_TO_OTHER_BORROWER) {
    const fromBody = readConflictBorrower(data);
    const fromSnapshot = snapshotBorrower(context.snapshot);
    const owner = fromBody?.userId ?? text(context.snapshot?.loanUserId);
    const patronId = text(context.patronId);
    const sameReader = Boolean(owner && patronId && owner === patronId);
    return {
      kind: 'onLoan',
      patronName: null,
      otherBorrower: sameReader ? null : (fromBody ?? fromSnapshot),
      loanId: null,
    };
  }
  if (code === ITEM_ALREADY_ON_LOAN_TO_BORROWER) {
    return {
      kind: 'alreadyOnLoan',
      patronName: null,
      loanId: renewLoanId(data, context),
    };
  }
  return null;
}

function renewLoanId(data: Record<string, unknown> | null, context: ConflictContext): string | null {
  const fromBody = readLoanId(data);
  if (fromBody) return fromBody;
  const owner = text(context.snapshot?.loanUserId);
  const patronId = text(context.patronId);
  const snapshotLoanId = text(context.snapshot?.loanId);
  if (snapshotLoanId && (!owner || owner === patronId)) return snapshotLoanId;
  return text(context.listedLoanId);
}

/**
 * These 409s are final. A checkout must not be retried with `force`.
 * A 422 is forceable only when its `forceable` field is boolean true.
 * A missing field is not forceable. The code and the caller's role are ignored.
 * `canForce` is loans write.
 */
export function checkoutMayForce(error: unknown, canForce = true): boolean {
  return loanRefusalMayForce(responseOf(error).data?.forceable, canForce);
}

/** `force` is omitted when the copy is already on loan. */
export function checkoutForceFlag(
  availability: string | null | undefined,
  force: boolean,
): true | undefined {
  if (availability === 'onLoan' || !force) return undefined;
  return true;
}
