import type {
  ItemWithdrawal,
  WithdrawalDestination,
  WithdrawalListParams,
  WithdrawalReason,
  WithdrawalWrite,
} from '@/types';

/** Closed reason list from the #139 contract. */
export const WITHDRAWAL_REASONS: readonly WithdrawalReason[] = [
  'worn',
  'obsolete',
  'newEdition',
  'surplus',
  'notBorrowed',
  'outOfPolicy',
  'lostNotReplaced',
  'other',
];

/** Closed destination list. `null` exists only on legacy rows. */
export const WITHDRAWAL_DESTINATIONS: readonly WithdrawalDestination[] = [
  'pulp',
  'donation',
  'sale',
  'deposit',
];

export const WITHDRAWAL_DETAIL_MIN = 3;

/**
 * Error `code` values from the #139 contract, including disposal (contract v2).
 * `authorization_failed`, `not_found`, and `validation_error` already have generic copy.
 *
 * Dispose batch (`POST /item-withdrawals/dispose`):
 * - 409 `withdrawal_dispose_rejected` when every rejected id is cancelled or already disposed
 * - 422 `withdrawal_dispose_rejected` when any id is unknown
 * - `failures[]` carries per-id `withdrawal_not_active`, `withdrawal_already_disposed`, or `not_found`
 * - 409 `withdrawal_already_disposed` when cancelling a row whose `disposedAt` is set
 */
export const WITHDRAWAL_API_ERROR_CODES = [
  'withdrawal_reason_detail_required',
  'withdrawal_destination_required',
  'withdrawal_already_active',
  'withdrawal_not_active',
  'withdrawal_already_disposed',
  'withdrawal_dispose_rejected',
  'item_on_loan',
  'item_in_transit',
  'item_on_hold',
  'withdrawn_via_withdrawal',
  'withdrawal_use_cancel',
  'withdrawal_not_legacy',
  'item_has_history_use_weeding',
  'biblio_has_items',
  'biblio_has_holds',
] as const;

export type WithdrawalApiErrorCode = (typeof WITHDRAWAL_API_ERROR_CODES)[number];

const CATALOG_DELETION_REFUSAL_CODES = [
  'item_has_history_use_weeding',
  'biblio_has_items',
  'biblio_has_holds',
] as const;

export function isCatalogDeletionRefusal(code: string | null | undefined): boolean {
  return !!code && (CATALOG_DELETION_REFUSAL_CODES as readonly string[]).includes(code);
}

export type WithdrawalFieldError = 'reason' | 'detail' | 'destination';

export interface WithdrawalFormInput {
  reason: WithdrawalReason | '';
  reasonDetail: string;
  destination: WithdrawalDestination | '';
}

export function validateWithdrawalWrite(
  input: WithdrawalFormInput,
): { ok: true; body: WithdrawalWrite } | { ok: false; field: WithdrawalFieldError } {
  if (!input.reason) return { ok: false, field: 'reason' };
  const detail = input.reasonDetail.trim();
  if (input.reason === 'other' && detail.length < WITHDRAWAL_DETAIL_MIN) {
    return { ok: false, field: 'detail' };
  }
  if (!input.destination) return { ok: false, field: 'destination' };
  return {
    ok: true,
    body: {
      reason: input.reason,
      reasonDetail: detail.length > 0 ? detail : null,
      destination: input.destination,
    },
  };
}

export function isWithdrawalDisposed(row: { disposedAt?: string | null } | null | undefined): boolean {
  return !!row?.disposedAt;
}

/** Cancel is allowed only on an active row that has not been disposed. */
export function canCancelWithdrawal(row: Pick<ItemWithdrawal, 'cancelledAt' | 'disposedAt'> | null | undefined): boolean {
  if (!row || row.cancelledAt) return false;
  return !isWithdrawalDisposed(row);
}

/** Legacy rows may be completed later, including after they were marked disposed on import. */
export function canCompleteLegacyWithdrawal(
  row: Pick<ItemWithdrawal, 'legacy' | 'cancelledAt'> | null | undefined,
): boolean {
  return !!row && row.legacy && !row.cancelledAt;
}

/** Eligible for the batch “mark disposed” action. */
export function canMarkDisposed(row: Pick<ItemWithdrawal, 'cancelledAt' | 'disposedAt'> | null | undefined): boolean {
  return canCancelWithdrawal(row);
}

export type DisposedFilter = 'all' | 'yes' | 'no';

export interface WithdrawalListFilters {
  from: string;
  to: string;
  siteId: string;
  reason: WithdrawalReason | '';
  destination: WithdrawalDestination | 'unset' | '';
  collectionId: string;
  callNumber: string;
  includeCancelled: boolean;
  disposed: DisposedFilter;
  page: number;
  perPage: number;
}

export function emptyWithdrawalFilters(): WithdrawalListFilters {
  return {
    from: '',
    to: '',
    siteId: '',
    reason: '',
    destination: '',
    collectionId: '',
    callNumber: '',
    includeCancelled: false,
    disposed: 'all',
    page: 1,
    perPage: 20,
  };
}

/** Local calendar day → RFC3339. `yyyy-mm-dd` compared as strings is chronological. */
export function localDayBoundIso(date: string, bound: 'start' | 'end'): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return null;
  const [year, month, day] = date.split('-').map(Number);
  if (!year || !month || !day) return null;
  const value =
    bound === 'start'
      ? new Date(year, month - 1, day, 0, 0, 0, 0)
      : new Date(year, month - 1, day, 23, 59, 59, 999);
  if (Number.isNaN(value.getTime())) return null;
  const check = new Date(value.getTime());
  if (check.getFullYear() !== year || check.getMonth() !== month - 1 || check.getDate() !== day) {
    return null;
  }
  return value.toISOString();
}

export function exportPeriodError(from: string, to: string): 'missing' | 'order' | null {
  if (!from || !to) return 'missing';
  if (from > to) return 'order';
  return null;
}

export function csvExportLanguage(language: string | null | undefined): 'french' | 'english' {
  return (language ?? '').toLowerCase().startsWith('fr') ? 'french' : 'english';
}

export function toWithdrawalListParams(
  filters: WithdrawalListFilters,
  options?: { forExport?: boolean; lang?: 'french' | 'english' },
): WithdrawalListParams {
  const params: WithdrawalListParams = {};
  const from = filters.from ? localDayBoundIso(filters.from, 'start') : null;
  const to = filters.to ? localDayBoundIso(filters.to, 'end') : null;
  if (from) params.from = from;
  if (to) params.to = to;
  if (filters.siteId) params.siteId = filters.siteId;
  if (filters.reason) params.reason = filters.reason;
  if (filters.destination) params.destination = filters.destination;
  if (filters.collectionId) params.collectionId = filters.collectionId;
  const callNumber = filters.callNumber.trim();
  if (callNumber) params.callNumber = callNumber;
  if (filters.includeCancelled) params.includeCancelled = true;
  if (filters.disposed === 'yes') params.disposed = true;
  if (filters.disposed === 'no') params.disposed = false;
  if (!options?.forExport) {
    params.page = filters.page;
    params.perPage = filters.perPage;
  }
  if (options?.lang) params.lang = options.lang;
  return params;
}

export function withdrawalActorName(row: Pick<ItemWithdrawal, 'disposedByName' | 'disposedBy'>): string | null {
  const name = row.disposedByName?.trim();
  if (name) return name;
  const id = row.disposedBy?.trim();
  return id || null;
}

/** Staff member who recorded the withdrawal (`agentName`, then the id). */
export function withdrawalAgentName(row: Pick<ItemWithdrawal, 'agentName' | 'agentId'>): string | null {
  const name = row.agentName?.trim();
  if (name) return name;
  const id = row.agentId?.trim();
  return id || null;
}

export interface DisposeFailure {
  id: string;
  code: string;
  message: string;
}

/** Per-id rows on a rejected `POST /item-withdrawals/dispose` body. */
export function parseDisposeFailures(data: unknown): DisposeFailure[] {
  if (!data || typeof data !== 'object') return [];
  const raw = (data as { failures?: unknown }).failures;
  if (!Array.isArray(raw)) return [];
  const failures: DisposeFailure[] = [];
  for (const entry of raw) {
    if (!entry || typeof entry !== 'object') continue;
    const row = entry as { id?: unknown; code?: unknown; message?: unknown };
    const id = row.id != null ? String(row.id).trim() : '';
    const code = typeof row.code === 'string' ? row.code.trim() : '';
    if (!id || !code) continue;
    failures.push({
      id,
      code,
      message: typeof row.message === 'string' ? row.message.trim() : '',
    });
  }
  return failures;
}
