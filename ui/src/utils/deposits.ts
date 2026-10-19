import type { TFunction } from 'i18next';
import type { ItemDeposit } from '@/types';
import { getHttpStatus } from '@/utils/apiError';
import { formatLocaleDate } from '@/utils/localeDate';

/** Checkout and renewal codes that must never offer « Forcer ». */
const NOT_FORCEABLE = new Set(['deposit_return_due', 'deposit_renewal_no_extension']);

export type DepositStockFilter = 'all' | 'own' | 'deposits';

export interface DepositCopyRef {
  deposit?: ItemDeposit | null;
}

export interface MarcBatchImportParamsInput {
  batchId: string;
  sourceId?: string | number | null;
  depositBatchId?: string | null;
  recordId?: number;
  allowDuplicateIsbn?: boolean;
  confirmReplaceExistingId?: string | number | boolean | null;
}

export interface DepositImportAttached {
  key: string;
  biblioId: string;
}

export interface DepositImportSummary {
  created: string[];
  attached: DepositImportAttached[];
}

export interface DepositCloseLine {
  barcode: string | null;
  title: string | null;
}

export interface DepositCloseReport {
  returned: DepositCloseLine[];
  missing: DepositCloseLine[];
}

export interface DueDateCarrier {
  dueDateCappedByDeposit?: boolean | null;
  expiryAt?: string | null;
}

/** `YYYY-MM-DD` stays on that calendar day. Instants use the active locale. */
export function formatCatalogDate(iso: string, language: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso.trim());
  if (!match) return formatLocaleDate(iso, language);
  const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]));
  return new Intl.DateTimeFormat(language, { dateStyle: 'long' }).format(date);
}

export function isDepositCopy(deposit: ItemDeposit | null | undefined): deposit is ItemDeposit {
  return deposit != null && deposit.batchId.trim() !== '' && deposit.lenderName.trim() !== '';
}

/** One badge per batch, in first-seen order. */
export function uniqueDeposits(copies: DepositCopyRef[] | null | undefined): ItemDeposit[] {
  const seen = new Set<string>();
  const out: ItemDeposit[] = [];
  for (const copy of copies ?? []) {
    if (!isDepositCopy(copy.deposit)) continue;
    if (seen.has(copy.deposit.batchId)) continue;
    seen.add(copy.deposit.batchId);
    out.push(copy.deposit);
  }
  return out;
}

/** « Dépôt · lender · retour le … ». Null for own stock. */
export function depositBadgeText(
  deposit: ItemDeposit | null | undefined,
  t: TFunction,
  language: string,
): string | null {
  if (!isDepositCopy(deposit)) return null;
  return t('deposits.badge', {
    lender: deposit.lenderName,
    date: formatCatalogDate(deposit.returnDueOn, language),
  });
}

/**
 * Client-side stock filter. The biblio list endpoint has no deposit query,
 * so this only sees copies already loaded.
 */
export function biblioMatchesDepositFilter(
  biblio: { items?: DepositCopyRef[] | null },
  filter: DepositStockFilter,
  lenderId?: string | null,
): boolean {
  const copies = biblio.items ?? [];
  const lender = lenderId?.trim() ?? '';
  if (filter === 'own') return copies.some((copy) => !isDepositCopy(copy.deposit));
  if (filter === 'deposits' || lender) {
    return copies.some((copy) => {
      if (!isDepositCopy(copy.deposit)) return false;
      return lender === '' || copy.deposit.lenderId === lender;
    });
  }
  return true;
}

export function filterLoadedBiblios<T extends { items?: DepositCopyRef[] | null }>(
  rows: T[],
  filter: DepositStockFilter,
  lenderId?: string | null,
): T[] {
  if (filter === 'all' && !(lenderId?.trim())) return rows;
  return rows.filter((row) => biblioMatchesDepositFilter(row, filter, lenderId));
}

/**
 * Query string for `POST /biblios/import-marc-batch`.
 * Deposit mode sends `deposit_batch_id` and omits the site and the duplicate-ISBN options.
 * `recordId` stays, so one cached notice can still be imported alone.
 */
export function buildMarcBatchImportParams(input: MarcBatchImportParamsInput): Record<string, unknown> {
  const params: Record<string, unknown> = { batchId: input.batchId };
  if (input.recordId != null) params.recordId = input.recordId;
  const depositId = input.depositBatchId?.toString().trim() ?? '';
  if (depositId) {
    params.deposit_batch_id = depositId;
    return params;
  }
  if (input.sourceId != null && String(input.sourceId) !== '') params.sourceId = input.sourceId;
  if (input.allowDuplicateIsbn) params.allowDuplicateIsbn = true;
  const confirm = input.confirmReplaceExistingId;
  if (confirm === true) params.confirmReplaceExistingId = true;
  else if (confirm != null && confirm !== false && String(confirm) !== '') {
    params.confirmReplaceExistingId = confirm;
  }
  return params;
}

export function summarizeDepositImportReport(result: unknown): DepositImportSummary | null {
  if (!result || typeof result !== 'object') return null;
  const body = result as Record<string, unknown>;
  const created = Array.isArray(body.created)
    ? body.created.filter((value): value is string => typeof value === 'string' && value.length > 0)
    : [];
  const attached = Array.isArray(body.attached)
    ? body.attached.flatMap((raw) => {
        if (!raw || typeof raw !== 'object') return [];
        const row = raw as Record<string, unknown>;
        const biblioId = row.biblioId ?? row.biblio_id;
        if (biblioId == null || String(biblioId) === '') return [];
        return [{ key: String(row.key ?? ''), biblioId: String(biblioId) }];
      })
    : [];
  if (created.length === 0 && attached.length === 0) return null;
  return { created, attached };
}

export function depositDueDateNotice(
  response: DueDateCarrier | null | undefined,
  t: TFunction,
  language: string,
): string | null {
  if (response?.dueDateCappedByDeposit !== true) return null;
  const iso = response.expiryAt?.trim();
  if (!iso) return t('deposits.dueDateCappedUnknown');
  return t('deposits.dueDateCapped', { date: formatCatalogDate(iso, language) });
}

export function withDepositDueNotice(
  base: string,
  response: DueDateCarrier | null | undefined,
  t: TFunction,
  language: string,
): string {
  const notice = depositDueDateNotice(response, t, language);
  return notice ? `${base} ${notice}` : base;
}

/** 409 on lender create or rename: the active name is already taken. */
export function lenderNameConflictMessage(error: unknown, t: TFunction): string | null {
  if (getHttpStatus(error) !== 409) return null;
  return t('deposits.lenders.nameConflict');
}

export function depositErrorBlocksForce(code: string | null | undefined): boolean {
  return code != null && NOT_FORCEABLE.has(code.trim());
}

const DEPOSIT_ERROR_CODES = [
  'deposit_return_due',
  'deposit_renewal_no_extension',
  'deposit_batch_returning',
  'deposit_item_not_owned',
  'deposit_site_mismatch',
  'deposit_batch_not_active',
  'deposit_import_options_conflict',
  'lender_has_active_batches',
  'barcode_already_exists',
] as const;

export function depositErrorMessageKey(code: string | null | undefined): string | null {
  const key = code?.trim() ?? '';
  if ((DEPOSIT_ERROR_CODES as readonly string[]).includes(key)) return `errors.apiCode.${key}`;
  return null;
}

export function marcImportFailureMessage(
  failure: { code?: string | null; error?: string | null },
  t: TFunction,
): string {
  const mapped = depositErrorMessageKey(failure.code);
  if (mapped) {
    const translated = t(mapped);
    if (translated !== mapped) return translated;
  }
  const raw = failure.error?.trim();
  if (raw) return raw;
  return t('importMarc.importErrorGeneric');
}

function closeLine(raw: unknown): DepositCloseLine | null {
  if (!raw || typeof raw !== 'object') return null;
  const row = raw as Record<string, unknown>;
  const barcode = typeof row.barcode === 'string' && row.barcode.trim() ? row.barcode.trim() : null;
  const title = typeof row.title === 'string' && row.title.trim() ? row.title.trim() : null;
  if (!barcode && !title) return null;
  return { barcode, title };
}

/**
 * Close report for the lender. Only barcode and title are kept.
 * Reader identity on the payload is dropped.
 */
export function parseDepositCloseReport(body: unknown): DepositCloseReport | null {
  if (!body || typeof body !== 'object') return null;
  const row = body as Record<string, unknown>;
  if (!('returned' in row) && !('missing' in row)) return null;
  const returned = Array.isArray(row.returned) ? row.returned.flatMap((line) => {
    const parsed = closeLine(line);
    return parsed ? [parsed] : [];
  }) : [];
  const missing = Array.isArray(row.missing) ? row.missing.flatMap((line) => {
    const parsed = closeLine(line);
    return parsed ? [parsed] : [];
  }) : [];
  return { returned, missing };
}

/**
 * Scan-out is idempotent. `alreadyScanned` is not on the published PR 1 contract;
 * the UI also treats a copy already in the returned list as already scanned.
 */
export function depositScanAlreadyReturned(body: unknown, wasAlreadyListed: boolean): boolean {
  if (wasAlreadyListed) return true;
  if (!body || typeof body !== 'object') return false;
  const row = body as Record<string, unknown>;
  return row.alreadyScanned === true || row.alreadyReturned === true;
}
