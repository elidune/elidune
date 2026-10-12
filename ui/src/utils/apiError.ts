import type { TFunction } from 'i18next';
import axios, { type AxiosError, type AxiosResponse } from 'axios';
import i18n from '@/locales';
import type { ApiError, MyLoanRenewalRefusal } from '@/types';
import { isLibraryLanguageValidationError } from '@/utils/uiLanguage';
import { weedingServerMessageI18nKey } from '@/utils/weeding';
import { parseDisposeFailures } from '@/utils/withdrawals';

/** Calendar date embedded in `Renewal is possible from YYYY-MM-DD.` */
const RENEWAL_OPEN_DATE = /(\d{4}-\d{2}-\d{2})/;

const RETRY_AFTER_MIN_MS = 250;
const RETRY_AFTER_MAX_MS = 8_000;
const RETRY_AFTER_DEFAULT_MS = 1_000;

function asAxiosError(error: unknown): AxiosError<ApiError> | null {
  return axios.isAxiosError<ApiError>(error) ? error : null;
}

export function getHttpStatus(error: unknown): number | null {
  const status = asAxiosError(error)?.response?.status;
  return typeof status === 'number' ? status : null;
}

/**
 * Field message for PUT /library-info timezone failures:
 * `invalid_timezone`, `timezone_country_mismatch`, and `timezone_required`.
 * Returns null for every other failure so the form can keep a generic banner.
 */
export function libraryInfoTimezoneFieldError(error: unknown, t: TFunction): string | null {
  const code = getApiErrorCode(error);
  if (code === 'invalid_timezone') return t('errors.apiCode.invalid_timezone');
  if (code === 'timezone_country_mismatch') return t('errors.apiCode.timezone_country_mismatch');
  if (code === 'timezone_required') return t('errors.apiCode.timezone_required');
  return null;
}

/**
 * Inline message for PUT /library-info when `language` is rejected with HTTP 400
 * `validation_error`. Other 400s stay on the form banner.
 */
export function libraryInfoLanguageFieldError(error: unknown, t: TFunction): string | null {
  if (!isLibraryLanguageValidationError({
    status: getHttpStatus(error),
    code: getApiErrorCode(error),
    message: getApiErrorRawMessage(error),
  })) {
    return null;
  }
  return t('library.general.defaultLanguageInvalid');
}

export function getApiErrorCode(error: unknown): string | null {
  const data = asAxiosError(error)?.response?.data;
  if (typeof data?.code === 'string' && data.code.trim() !== '') {
    return data.code.trim();
  }
  return null;
}

/** 401, or 403 whose code says the session itself is invalid (not a permission denial). */
export function isInvalidSessionError(error: unknown): boolean {
  const status = getHttpStatus(error);
  if (status === 401) return true;
  return status === 403 && getApiErrorCode(error) === 'authentication_failed';
}

/** HTTP 429 or API code `rate_limited` (auth governor and any other limiter). */
export function isRateLimitedError(error: unknown): boolean {
  if (getHttpStatus(error) === 429) return true;
  return getApiErrorCode(error) === 'rate_limited';
}

function headerValue(response: AxiosResponse | undefined, name: string): unknown {
  const headers = response?.headers;
  if (!headers) return undefined;
  if (typeof headers.get === 'function') {
    return headers.get(name);
  }
  const record = headers as unknown as Record<string, unknown>;
  return record[name] ?? record[name.toLowerCase()];
}

function parseRetryAfterValue(raw: unknown, now: number): number | null {
  const value = Array.isArray(raw) ? raw[0] : raw;
  if (typeof value === 'number' && Number.isFinite(value)) {
    return Math.max(0, Math.ceil(value));
  }
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (/^\d+(\.\d+)?$/.test(trimmed)) {
    return Math.max(0, Math.ceil(Number(trimmed)));
  }
  const at = Date.parse(trimmed);
  if (Number.isNaN(at)) return null;
  return Math.max(0, Math.ceil((at - now) / 1000));
}

/**
 * Seconds to wait from a 429 `Retry-After` header (delta-seconds or HTTP-date),
 * then common JSON fields (`retryAfter`, `retry_after`).
 */
export function getRetryAfterSeconds(error: unknown, now = Date.now()): number | null {
  const response = asAxiosError(error)?.response;
  const fromHeader = parseRetryAfterValue(headerValue(response, 'retry-after'), now);
  if (fromHeader != null) return fromHeader;

  const data = response?.data;
  if (!data || typeof data !== 'object' || isBlobLike(data)) return null;
  const record = data as unknown as Record<string, unknown>;
  return parseRetryAfterValue(record.retryAfter ?? record.retry_after, now);
}

/** Honor `Retry-After`, clamped so a few profile retries cannot stall the UI. */
export function retryAfterDelayMs(error: unknown, now = Date.now()): number {
  const seconds = getRetryAfterSeconds(error, now);
  const ms = seconds == null ? RETRY_AFTER_DEFAULT_MS : seconds * 1000;
  return Math.min(RETRY_AFTER_MAX_MS, Math.max(RETRY_AFTER_MIN_MS, ms));
}

const RETRY_WAIT_MINUTE_SECONDS = 60;

/** Rounded wait used in the rate-limit message: seconds under a minute, otherwise minutes. */
export function retryWaitAmount(seconds: number): { unit: 'second' | 'minute'; count: number } {
  const safe = Math.max(0, Math.ceil(seconds));
  if (safe < RETRY_WAIT_MINUTE_SECONDS) return { unit: 'second', count: safe };
  return {
    unit: 'minute',
    count: Math.max(1, Math.round(safe / RETRY_WAIT_MINUTE_SECONDS)),
  };
}

/** Locale unit phrase (`45 seconds`, `11 min`) via `Intl.NumberFormat`. */
export function formatRetryWaitDuration(seconds: number, language: string): string {
  const { unit, count } = retryWaitAmount(seconds);
  return new Intl.NumberFormat(language, {
    style: 'unit',
    unit,
    unitDisplay: unit === 'minute' ? 'short' : 'long',
  }).format(count);
}

/**
 * Human rate-limit wait. Under 60s the duration is exact seconds; longer waits
 * round to minutes (`about 11 min`). Call this whenever the shown wait changes,
 * including a countdown, so the unit stays in step with the number.
 */
export function rateLimitWaitMessage(seconds: number, t: TFunction, language?: string): string {
  const lang = language || i18n.resolvedLanguage || i18n.language || 'en';
  const { unit, count } = retryWaitAmount(seconds);
  const duration = formatRetryWaitDuration(seconds, lang);
  const key =
    unit === 'second' ? 'errors.tooManyRequestsWaitSeconds' : 'errors.tooManyRequestsWaitMinutes';
  return t(key, { count, duration });
}

export function rateLimitMessage(error: unknown, t: TFunction): string {
  const seconds = getRetryAfterSeconds(error);
  if (seconds != null) return rateLimitWaitMessage(seconds, t);
  return t('errors.tooManyRequests');
}

/**
 * Login form mapping: 401 invalid credentials, 429 / `rate_limited` with the
 * `Retry-After` wait, network and 5xx as a generic error.
 */
export function loginFailureMessage(error: unknown, t: TFunction): string {
  if (isRateLimitedError(error)) return rateLimitMessage(error, t);
  const status = getHttpStatus(error);
  if (status === 401) return t('auth.invalidCredentials');
  if (status == null || status >= 500) return t('errors.generic');
  return getApiErrorMessage(error, t);
}

/**
 * Opening date from a 422 `renewal_too_early` body.
 * The server puts it in `message` (`Renewal is possible from YYYY-MM-DD.`);
 * there is no separate date field.
 */
export function renewalTooEarlyOpenDate(error: unknown): string | null {
  if (getApiErrorCode(error) !== 'renewal_too_early') return null;
  const message = getApiErrorRawMessage(error);
  return message?.match(RENEWAL_OPEN_DATE)?.[1] ?? null;
}

/** Account-wide refusals. The server checks these before any per-loan reason. */
const ACCOUNT_RENEWAL_REFUSALS = ['account_blocked', 'fines_over_threshold'] as const;

const RENEWAL_REFUSAL_CODES: readonly MyLoanRenewalRefusal[] = [
  ...ACCOUNT_RENEWAL_REFUSALS,
  'withdrawn',
  'claimed_returned',
  'hold_waiting',
  'max_renewals',
  'renewal_too_early',
];

export function isRenewalRefusalCode(code: string | null | undefined): code is MyLoanRenewalRefusal {
  return !!code && (RENEWAL_REFUSAL_CODES as readonly string[]).includes(code);
}

export function isAccountRenewalRefusal(
  code: string | null | undefined,
): code is (typeof ACCOUNT_RENEWAL_REFUSALS)[number] {
  return code === 'account_blocked' || code === 'fines_over_threshold';
}

export function formatIsoCalendarDate(isoDate: string, language: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(isoDate);
  if (!match) return isoDate;
  const date = new Date(Date.UTC(Number(match[1]), Number(match[2]) - 1, Number(match[3])));
  return date.toLocaleDateString(language, { timeZone: 'UTC' });
}

/**
 * Same sentence for a list row and for a POST `/loans/{id}/renew` error.
 * `renewal_too_early` without a date falls back to the dateless sentence.
 * `card_lost` on the patron list asks them to come to the desk. The staff
 * checkout error uses `errors.apiCode.card_lost` instead, so it is not an
 * API-error refusal code here.
 */
export function renewalRefusalText(
  t: TFunction,
  code: string | null | undefined,
  opensAt: string | null | undefined,
  language: string,
): string | null {
  if (code === 'card_lost') return t('loans.renewalRefusal.card_lost');
  if (!isRenewalRefusalCode(code)) return null;
  if (code === 'renewal_too_early') {
    if (!opensAt) return t('loans.renewalTooEarly');
    return t('loans.renewalRefusal.renewal_too_early', {
      date: formatIsoCalendarDate(opensAt, language),
    });
  }
  return t(`loans.renewalRefusal.${code}`);
}

/** Localized refusal for a renew POST. `renewal_too_early` reads the date from the body. */
export function renewalRefusalErrorMessage(error: unknown, t: TFunction): string | null {
  const code = getApiErrorCode(error);
  if (!isRenewalRefusalCode(code)) return null;
  const language = i18n.resolvedLanguage || i18n.language || 'en';
  const opensAt = code === 'renewal_too_early' ? renewalTooEarlyOpenDate(error) : null;
  return renewalRefusalText(t, code, opensAt, language);
}

/** Localized "Renewal possible from {date}" for `renewal_too_early`. */
export function renewalTooEarlyMessage(error: unknown, t: TFunction): string | null {
  if (getApiErrorCode(error) !== 'renewal_too_early') return null;
  return renewalRefusalErrorMessage(error, t);
}

/** Server `message` / `error` body text, if present. */
export function getApiErrorRawMessage(error: unknown): string | null {
  const data = (error as AxiosError<ApiError>)?.response?.data;
  if (!data || typeof data !== 'object' || isBlobLike(data)) return null;
  const d = data as ApiError;
  if (typeof d.message === 'string' && d.message.trim()) return d.message.trim();
  if (typeof d.error === 'string' && d.error.trim()) return d.error.trim();
  return null;
}

function isBlobLike(data: unknown): data is Blob {
  return typeof Blob !== 'undefined' && data instanceof Blob;
}

/**
 * i18n key for an HTTP status when the response body has no usable message.
 * Covers common REST / proxy status codes; unknown 4xx / 5xx fall back to clientError / serverError.
 */
function httpStatusToMessageKey(status: number): string | null {
  if (status === 429) return 'errors.tooManyRequests';
  const specific: Record<number, string> = {
    400: 'errors.http.badRequest',
    401: 'errors.http.unauthorized',
    402: 'errors.http.paymentRequired',
    403: 'errors.http.forbidden',
    404: 'errors.http.notFound',
    405: 'errors.http.methodNotAllowed',
    406: 'errors.http.notAcceptable',
    408: 'errors.http.requestTimeout',
    409: 'errors.http.conflict',
    410: 'errors.http.gone',
    411: 'errors.http.lengthRequired',
    412: 'errors.http.preconditionFailed',
    413: 'errors.http.payloadTooLarge',
    414: 'errors.http.uriTooLong',
    415: 'errors.http.unsupportedMedia',
    416: 'errors.http.rangeNotSatisfiable',
    421: 'errors.http.misdirectedRequest',
    422: 'errors.http.unprocessable',
    423: 'errors.http.locked',
    424: 'errors.http.failedDependency',
    426: 'errors.http.upgradeRequired',
    428: 'errors.http.preconditionRequired',
    431: 'errors.http.requestHeaderFieldsTooLarge',
    451: 'errors.http.unavailableForLegalReasons',
    501: 'errors.http.notImplemented',
    507: 'errors.http.insufficientStorage',
    511: 'errors.http.networkAuthRequired',
  };
  if (specific[status]) return specific[status];
  if (status >= 500 && status <= 599) {
    if (status === 502 || status === 503 || status === 504) return 'errors.http.badGateway';
    return 'errors.http.serverError';
  }
  if (status >= 400 && status <= 499) return 'errors.http.clientError';
  return null;
}

function translateKey(t: TFunction, key: string): string | null {
  const msg = t(key);
  return msg === key ? null : msg;
}

/**
 * Get a user-facing error message from an API error response.
 * Order: rate limit (429) → API error code → response body message → HTTP status → network / timeout → generic.
 */
/**
 * Dispose-batch message. 422 and 409 share `withdrawal_dispose_rejected`;
 * `failures` lists each id with its own code.
 */
export function disposeBatchErrorMessage(error: unknown, t: TFunction): string {
  const code = getApiErrorCode(error);
  const status = getHttpStatus(error);
  const head =
    code === 'withdrawal_dispose_rejected' && status === 422
      ? t('errors.apiCode.withdrawal_dispose_rejected_unknown')
      : getApiErrorMessage(error, t);
  const data = asAxiosError(error)?.response?.data;
  const lines = parseDisposeFailures(data).map((failure) => {
    const fromCode = translateKey(t, `errors.apiCode.${failure.code}`);
    const detail = fromCode ?? (failure.message || failure.code);
    return t('weeding.disposeFailure', { id: failure.id, detail });
  });
  return lines.length > 0 ? [head, ...lines].join('\n') : head;
}

export function getApiErrorMessage(error: unknown, t: TFunction): string {
  const axiosError = asAxiosError(error);
  const status = axiosError?.response?.status;
  const rawData = axiosError?.response?.data;

  if (isRateLimitedError(error)) {
    return rateLimitMessage(error, t);
  }

  const weedingKey = weedingServerMessageI18nKey(getApiErrorRawMessage(error));
  if (weedingKey !== null) {
    const fromWeeding = translateKey(t, weedingKey);
    if (fromWeeding !== null) return fromWeeding;
  }

  const renewalRefusal = renewalRefusalErrorMessage(error, t);
  if (renewalRefusal !== null) return renewalRefusal;

  const code = getApiErrorCode(error);
  if (code !== null) {
    const fromCode = translateKey(t, `errors.apiCode.${code}`);
    if (fromCode !== null) return fromCode;
  }

  let data = rawData;
  if (isBlobLike(rawData)) {
    data = undefined;
  }

  if (data && typeof data === 'object' && !Array.isArray(data)) {
    const d = data as ApiError;
    if (typeof d.message === 'string' && d.message.trim()) return d.message.trim();
    if (typeof d.error === 'string' && d.error.trim()) return d.error.trim();
  }

  if (status != null) {
    const httpKey = httpStatusToMessageKey(status);
    if (httpKey) {
      const fromHttp = translateKey(t, httpKey);
      if (fromHttp !== null) return fromHttp;
    }
  }

  if (!axiosError?.response) {
    if (axiosError?.message === 'Network Error') return t('errors.network');
    if (axiosError?.code === 'ECONNABORTED') return t('errors.timeout');
    return t('errors.generic');
  }

  return t('errors.generic');
}
