import type { TFunction } from 'i18next';
import { getApiErrorCode, getApiErrorMessage, getApiErrorRawMessage, getHttpStatus } from '@/utils/apiError';

/** Maximum length of a hold `patronNote` or `internalNote` (Unicode scalar values). */
export const HOLD_NOTE_MAX_CHARS = 4000;

/** Server 403 `authorization_failed` when `internalNote` is sent without holds write. */
export const INTERNAL_HOLD_NOTE_FORBIDDEN = 'Insufficient rights to set an internal hold note';

/** Unicode code points (`[...str].length`), matching the server `chars().count()` limit. */
export function holdNoteLength(value: string): number {
  return [...value].length;
}

/** Keep at most {@link HOLD_NOTE_MAX_CHARS} code points. */
export function clampHoldNote(value: string): string {
  const chars = [...value];
  if (chars.length <= HOLD_NOTE_MAX_CHARS) return value;
  return chars.slice(0, HOLD_NOTE_MAX_CHARS).join('');
}

export function holdNoteWithinLimit(value: string): boolean {
  return holdNoteLength(value) <= HOLD_NOTE_MAX_CHARS;
}

function storedNote(value: string | null | undefined): string {
  return (value ?? '').trim();
}

/**
 * One note on `POST /holds`.
 * `undefined` omits the field (unchanged).
 * `""` clears a stored note. A changed value is sent trimmed.
 */
export function holdNoteWrite(stored: string | null | undefined, draft: string): string | undefined {
  const next = storedNote(clampHoldNote(draft));
  if (next === storedNote(stored)) return undefined;
  return next;
}

export interface HoldNoteDraft {
  patronNote: string;
  internalNote: string;
}

export interface HoldNoteStored {
  patronNote?: string | null;
  internalNote?: string | null;
}

/**
 * Note fields for `POST /holds`.
 * Untouched notes are absent. `""` clears.
 * `internalNote` is included only when the caller has holds write.
 */
export function holdNotePayload(
  stored: HoldNoteStored,
  draft: HoldNoteDraft,
  options: { canWriteInternal: boolean },
): { patronNote?: string; internalNote?: string } {
  const payload: { patronNote?: string; internalNote?: string } = {};
  const patronNote = holdNoteWrite(stored.patronNote, draft.patronNote);
  if (patronNote !== undefined) payload.patronNote = patronNote;
  if (options.canWriteInternal) {
    const internalNote = holdNoteWrite(stored.internalNote, draft.internalNote);
    if (internalNote !== undefined) payload.internalNote = internalNote;
  }
  return payload;
}

/**
 * Body for `POST /holds`.
 * Drops the retired `notes` key (the server returns 400) and caps note fields by code point.
 * Callers without holds write never send `internalNote`.
 */
export function holdWritePayload<T extends object>(
  body: T,
  options: { canWriteInternal: boolean },
): Omit<T, 'notes'> {
  const out: Record<string, unknown> = { ...(body as Record<string, unknown>) };
  delete out.notes;
  if (typeof out.patronNote === 'string') out.patronNote = clampHoldNote(out.patronNote);
  if (!options.canWriteInternal) {
    delete out.internalNote;
  } else if (typeof out.internalNote === 'string') {
    out.internalNote = clampHoldNote(out.internalNote);
  }
  return out as Omit<T, 'notes'>;
}

/** Blank, null, and absent notes render as nothing. */
export function holdNoteText(value: string | null | undefined): string {
  return value?.trim() ?? '';
}

/** Read one optional note. Missing keys (search and public lists) stay absent. */
export function readOptionalHoldNote(
  row: object,
  key: 'patronNote' | 'internalNote',
): string | null | undefined {
  if (!(key in row)) return undefined;
  const value = (row as Record<string, unknown>)[key];
  if (typeof value === 'string' || value === null) return value;
  return undefined;
}

/**
 * Hold create failure. A 403 for `internalNote` without holds write is a specific
 * sentence. Every other failure keeps the shared API message.
 */
export function holdWriteErrorMessage(error: unknown, t: TFunction): string {
  const raw = getApiErrorRawMessage(error);
  if (
    getHttpStatus(error) === 403 &&
    getApiErrorCode(error) === 'authorization_failed' &&
    raw === INTERNAL_HOLD_NOTE_FORBIDDEN
  ) {
    return t('holds.internalNoteForbidden');
  }
  return getApiErrorMessage(error, t) || t('holds.createError');
}
