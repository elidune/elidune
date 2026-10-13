/** Maximum length of an item `publicNote` or `internalNote` (Unicode scalar values). */
export const COPY_NOTE_MAX_CHARS = 4000;

/** Unicode code points (`[...str].length`), matching the server `chars().count()` limit. */
export function copyNoteLength(value: string): number {
  return [...value].length;
}

/** Keep at most {@link COPY_NOTE_MAX_CHARS} code points. */
export function clampCopyNote(value: string): string {
  const chars = [...value];
  if (chars.length <= COPY_NOTE_MAX_CHARS) return value;
  return chars.slice(0, COPY_NOTE_MAX_CHARS).join('');
}

export function copyNoteWithinLimit(value: string): boolean {
  return copyNoteLength(value) <= COPY_NOTE_MAX_CHARS;
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

/**
 * Body for item create and `PUT /items/{id}`.
 * Drops the retired `notes` key (the server returns 400) and caps note fields by code point.
 */
export function itemWritePayload<T extends object>(body: T): Omit<T, 'notes'> {
  const out: Record<string, unknown> = { ...(body as Record<string, unknown>) };
  delete out.notes;
  if (typeof out.publicNote === 'string') out.publicNote = clampCopyNote(out.publicNote);
  if (typeof out.internalNote === 'string') out.internalNote = clampCopyNote(out.internalNote);
  return out as Omit<T, 'notes'>;
}

/** Strip retired copy `notes` from nested items. Bibliographic `notes` on the record itself stay. */
export function biblioWritePayload<T extends { items?: readonly object[] | null }>(body: T): T {
  if (!body.items) return body;
  return {
    ...body,
    items: body.items.map((item) => itemWritePayload(item)),
  };
}
