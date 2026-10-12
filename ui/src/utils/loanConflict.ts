import type { DeskCopySnapshot, DeskScanAlert } from './deskScanAlert.ts';

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

function readCode(error: unknown): string | null {
  return text(responseOf(error).data?.code);
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
 * The same codes are returned with or without `force`; neither opens a force retry.
 */
export function checkoutNoticeForConflict(error: unknown, context: ConflictContext): CheckoutLoanNotice | null {
  const { status, data } = responseOf(error);
  if (status !== 409) return null;
  const code = text(data?.code);
  if (code === ITEM_ON_LOAN_TO_OTHER_BORROWER) {
    const owner = text(context.snapshot?.loanUserId);
    const patronId = text(context.patronId);
    const otherReader = Boolean(owner && patronId && owner !== patronId);
    return {
      kind: 'onLoan',
      patronName: otherReader ? text(context.snapshot?.loanUserName) : null,
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

/** These 409s are final. A checkout must not be retried with `force`. */
export function checkoutMayForce(error: unknown): boolean {
  const code = readCode(error);
  if (code === ITEM_ON_LOAN_TO_OTHER_BORROWER || code === ITEM_ALREADY_ON_LOAN_TO_BORROWER) return false;
  return code === 'business_rule_violation';
}

/** `force` is omitted when the copy is already on loan. */
export function checkoutForceFlag(
  availability: string | null | undefined,
  force: boolean,
): true | undefined {
  if (availability === 'onLoan' || !force) return undefined;
  return true;
}
