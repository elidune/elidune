/** Maximum length of an item `publicNote` or `internalNote` (Unicode scalar values). */
export const COPY_NOTE_MAX_CHARS = 4000;

/** Unicode scalar values, matching the server `chars().count()` limit. */
export function copyNoteLength(value: string): number {
  return Array.from(value).length;
}

/** Keep at most {@link COPY_NOTE_MAX_CHARS} scalar values. */
export function clampCopyNote(value: string): string {
  const chars = Array.from(value);
  if (chars.length <= COPY_NOTE_MAX_CHARS) return value;
  return chars.slice(0, COPY_NOTE_MAX_CHARS).join('');
}

function storedNote(value: string | null | undefined): string {
  return (value ?? '').trim();
}

/**
 * One note on `PUT /items/{id}`.
 * `undefined` omits the field (the server keeps the stored note).
 * `""` clears it. A changed value is sent trimmed.
 */
export function copyNoteWrite(stored: string | null | undefined, draft: string): string | undefined {
  const next = storedNote(clampCopyNote(draft));
  if (next === storedNote(stored)) return undefined;
  return next;
}

export interface CopyNoteDraft {
  publicNote: string;
  internalNote: string;
}

export interface CopyNoteStored {
  publicNote?: string | null;
  internalNote?: string | null;
}

/** Note fields to merge into `PUT /items/{id}`. Untouched notes are absent. */
export function copyNoteUpdate(
  stored: CopyNoteStored,
  draft: CopyNoteDraft,
): { publicNote?: string; internalNote?: string } {
  const payload: { publicNote?: string; internalNote?: string } = {};
  const publicNote = copyNoteWrite(stored.publicNote, draft.publicNote);
  const internalNote = copyNoteWrite(stored.internalNote, draft.internalNote);
  if (publicNote !== undefined) payload.publicNote = publicNote;
  if (internalNote !== undefined) payload.internalNote = internalNote;
  return payload;
}
