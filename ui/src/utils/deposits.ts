import type { TFunction } from 'i18next';
import type { ItemDeposit } from '@/types';
import { getApiErrorCode, getApiErrorData, getHttpStatus } from '@/utils/apiError';
import { formatLocaleDate } from '@/utils/localeDate';

/** Checkout and renewal codes that must never offer « Forcer ». */
const NOT_FORCEABLE = new Set(['deposit_return_due', 'deposit_renewal_no_extension']);

export type DepositStockFilter = 'all' | 'own' | 'deposits';

export interface DepositCopyRef {
  deposit?: ItemDeposit | null;
}

export interface MarcBatchImportParamsInput {
  batchId: string;
  siteId?: string | number | null;
  depositBatchId?: string | null;
  recordId?: number;
  allowDuplicateIsbn?: boolean;
  confirmReplaceExistingId?: string | number | boolean | null;
  depositDuplicateAction?: 'overwrite' | 'relabel' | 'ignore';
  depositDuplicateApplyAll?: boolean;
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

/** One copy on `422 deposit_batch_has_active_loans`. The body has no title. */
export interface DepositActiveLoanCopy {
  itemId: string;
  barcode: string | null;
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

/** Two letters for deposit overlay on catalog document icons. */
export function depositLenderInitials(lenderName: string | null | undefined): string {
  const parts = String(lenderName ?? '')
    .trim()
    .split(/\s+/)
    .filter(Boolean);
  if (parts.length >= 2) {
    return `${parts[0]![0] ?? ''}${parts[1]![0] ?? ''}`.toUpperCase();
  }
  const word = parts[0] ?? '?';
  return word.slice(0, 2).toUpperCase();
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
  if (!deposit.returnDueOn?.trim()) {
    return t('deposits.badgePermanent', { lender: deposit.lenderName });
  }
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
    if (input.siteId != null && String(input.siteId) !== '') {
      params.siteId = input.siteId;
    }
    if (input.depositDuplicateAction) params.depositDuplicateAction = input.depositDuplicateAction;
    if (input.depositDuplicateApplyAll === true) params.depositDuplicateApplyAll = true;
    return params;
  }
  if (input.siteId != null && String(input.siteId) !== '') params.siteId = input.siteId;
  if (input.allowDuplicateIsbn) params.allowDuplicateIsbn = true;
  const confirm = input.confirmReplaceExistingId;
  if (confirm === true) params.confirmReplaceExistingId = true;
  else if (confirm != null && confirm !== false && String(confirm) !== '') {
    params.confirmReplaceExistingId = confirm;
  }
  return params;
}

/** Staff messages after deposit MARC import when copies were relabeled. */
export function depositMarcRelabelNotices(report: unknown, t: TFunction): string[] {
  if (!report || typeof report !== 'object') return [];
  const relabeled = (report as Record<string, unknown>).copiesRelabeled;
  if (!Array.isArray(relabeled)) return [];
  return relabeled.flatMap((raw) => {
    if (!raw || typeof raw !== 'object') return [];
    const row = raw as Record<string, unknown>;
    const lender = String(row.lenderBarcode ?? '').trim();
    const local = String(row.localBarcode ?? '').trim();
    if (!lender || !local) return [];
    return [t('deposits.import.relabelNotice', { lender, local })];
  });
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
  'deposit_batch_has_active_loans',
  'deposit_batch_not_returning',
  'deposit_replacement_amount_required',
  'item_not_deposit_copy',
  'deposit_item_on_loan',
  'deposit_loss_not_declared',
  'deposit_loss_already_notified',
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

function closeLines(value: unknown): DepositCloseLine[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((line) => {
    const parsed = closeLine(line);
    return parsed ? [parsed] : [];
  });
}

function record(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function cleanName(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const name = value.trim();
  return name || null;
}

/**
 * Close report for the lender. Only barcode and title are kept.
 * Reader identity is dropped. `onLoan` is not part of the report.
 */
export function parseDepositCloseReport(body: unknown): DepositCloseReport | null {
  const row = record(body);
  if (!row) return null;
  if (!('returned' in row) && !('missing' in row)) return null;
  return {
    returned: closeLines(row.returned),
    missing: closeLines(row.missing),
  };
}

function activeLoanCopy(raw: unknown): DepositActiveLoanCopy | null {
  const row = record(raw);
  if (!row) return null;
  const id = row.itemId ?? row.item_id;
  if (id == null || String(id).trim() === '') return null;
  const barcode = typeof row.barcode === 'string' && row.barcode.trim() ? row.barcode.trim() : null;
  return { itemId: String(id).trim(), barcode };
}

/**
 * Copies on `422 deposit_batch_has_active_loans`: `{ copies: [{ itemId, barcode }] }`.
 * Null for any other code. Title and reader fields are not read.
 */
export function depositActiveLoanCopies(error: unknown): DepositActiveLoanCopy[] | null {
  if (getApiErrorCode(error) !== 'deposit_batch_has_active_loans') return null;
  const copies = getApiErrorData(error)?.copies;
  if (!Array.isArray(copies)) return [];
  return copies.flatMap((line) => {
    const parsed = activeLoanCopy(line);
    return parsed ? [parsed] : [];
  });
}

/** Title and biblio id from the batch's already-loaded on-loan rows, matched by item id. */
export function activeLoanCopyLabel(
  copy: DepositActiveLoanCopy,
  loaded: { itemId: string; title?: string | null; biblioId?: string | null }[] | null | undefined,
): { title: string | null; biblioId: string | null } {
  const known = (loaded ?? []).find((item) => item.itemId === copy.itemId);
  const title = known?.title?.trim() || null;
  const biblioId = known?.biblioId?.trim() || null;
  return { title, biblioId };
}

export type ReplacementRateParse =
  | { ok: true; cents: number | null }
  | { ok: false; reason: 'invalid' | 'notPositive' };

/**
 * Euros typed in the lender form, stored as integer cents.
 * Empty is no rate. Zero and negative values are rejected: the server never stores them.
 */
export function parseReplacementRateEuros(raw: string): ReplacementRateParse {
  const text = raw.trim().replace(/\s/g, '').replace(',', '.');
  if (!text) return { ok: true, cents: null };
  if (!/^\d+(\.\d{1,2})?$/.test(text)) return { ok: false, reason: 'invalid' };
  const [euros, frac = ''] = text.split('.');
  const cents = Number(euros) * 100 + Number(frac.padEnd(2, '0'));
  if (!Number.isSafeInteger(cents)) return { ok: false, reason: 'invalid' };
  if (cents <= 0) return { ok: false, reason: 'notPositive' };
  return { ok: true, cents };
}

export interface LenderFormWrite {
  name: string;
  contact: string | null;
  notes: string | null;
  /** JSON null clears the rate. A positive integer sets it. Never zero. */
  replacementRateCents: number | null;
}

/**
 * Lender create and edit always send `replacementRateCents`.
 * An empty field is JSON `null` (clears the stored rate on edit).
 * Callers must reject zero before calling this.
 */
export function buildLenderWrite(input: {
  name: string;
  contact: string;
  notes: string;
  replacementRateCents: number | null;
}): LenderFormWrite {
  return {
    name: input.name.trim(),
    contact: input.contact.trim() || null,
    notes: input.notes.trim() || null,
    replacementRateCents: input.replacementRateCents,
  };
}

/** Staff line when the lender was told this copy was lost or damaged. */
export function depositLossNotifiedText(
  deposit: { depositLossNotifiedAt?: string | null } | null | undefined,
  t: TFunction,
  language: string,
): string | null {
  const iso = deposit?.depositLossNotifiedAt?.trim();
  if (!iso) return null;
  return t('deposits.losses.notifiedOn', { date: formatLocaleDate(iso, language) });
}

/** Editable euro string for a stored rate. Empty when there is no rate. */
export function replacementRateInputValue(cents: number | null | undefined): string {
  if (cents == null || !Number.isFinite(cents) || cents <= 0) return '';
  const whole = Math.trunc(cents / 100);
  const frac = Math.abs(cents % 100);
  return `${whole}.${String(frac).padStart(2, '0')}`;
}

export function formatReplacementRateCents(cents: number | null | undefined, locale: string): string | null {
  if (cents == null || !Number.isFinite(cents) || cents <= 0) return null;
  return new Intl.NumberFormat(locale, { style: 'currency', currency: 'EUR' }).format(cents / 100);
}

export function isPositiveEuroAmount(value: string): boolean {
  const amount = Number(value.trim().replace(',', '.'));
  return Number.isFinite(amount) && amount > 0;
}

/**
 * Lost or damaged deposit copy: a 422 makes the staff amount required.
 * Server order is lender rate, then a copy price above zero, then this amount. Zero is never a bill.
 */
export function replacementAmountFieldState(input: {
  kind: string;
  bill: boolean;
  amount: string;
  serverRequiresAmount: boolean;
}): { bill: boolean; required: boolean; invalid: boolean } {
  const mayBill = input.kind === 'lost' || input.kind === 'damaged';
  const required = mayBill && input.serverRequiresAmount;
  const bill = required || input.bill;
  const positive = isPositiveEuroAmount(input.amount);
  const invalid = required ? !positive : input.kind === 'damaged' && bill && !positive;
  return { bill, required, invalid };
}

export function depositReplacementAmountRequired(error: unknown): boolean {
  return getApiErrorCode(error) === 'deposit_replacement_amount_required';
}

interface NamedDepositCopy {
  id?: string | null;
  deposit?: { lenderName?: unknown } | null;
}

/** Lender name on a loan row's embedded copies. */
export function lenderNameFromCopies(
  copies: NamedDepositCopy[] | null | undefined,
  itemId?: string | null,
): string | null {
  const rows = copies ?? [];
  const id = itemId?.trim();
  const match = id ? rows.find((row) => row.id === id) : undefined;
  const named = (match ? [match] : rows)
    .map((row) => cleanName(row.deposit?.lenderName))
    .find((name): name is string => name != null);
  return named ?? null;
}

function lenderNameFromLoanRecord(value: unknown): string | null {
  const loan = record(value);
  if (!loan) return null;
  const item = record(loan.item);
  const fromItem = cleanName(record(item?.deposit)?.lenderName);
  if (fromItem) return fromItem;
  const biblio = record(loan.biblio);
  const itemId = typeof loan.itemId === 'string' ? loan.itemId : null;
  const copies = Array.isArray(biblio?.items) ? biblio.items.flatMap((row) => {
    const copy = record(row);
    if (!copy) return [];
    return [{ id: typeof copy.id === 'string' ? copy.id : null, deposit: record(copy.deposit) }];
  }) : [];
  return lenderNameFromCopies(copies, itemId);
}

/**
 * Desk notice when a check-in or scan-out says `returnToLender`.
 * The flag is true while the deposit batch is `returning`.
 * Null when the flag is absent or false.
 */
export function returnToLenderNotice(
  body: unknown,
  t: TFunction,
  fallbackLender?: string | null,
): string | null {
  const row = record(body);
  if (!row) return null;
  if (row.returnToLender !== true && row.return_to_lender !== true) return null;
  const lender = cleanName(row.lenderName)
    ?? cleanName(record(row.deposit)?.lenderName)
    ?? lenderNameFromLoanRecord(row.loan)
    ?? cleanName(fallbackLender);
  if (!lender) return t('deposits.returnToLenderUnknown');
  return t('deposits.returnToLender', { lender });
}

export function withReturnToLenderNotice(
  base: string,
  body: unknown,
  t: TFunction,
  fallbackLender?: string | null,
): string {
  const notice = returnToLenderNotice(body, t, fallbackLender);
  return notice ? `${base} ${notice}` : base;
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
