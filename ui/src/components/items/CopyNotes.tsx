import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';
import { COPY_NOTE_MAX_CHARS, clampCopyNote, copyNoteLength } from '@/utils/itemNotes';

interface CopyNoteFieldsProps {
  publicNote: string;
  internalNote: string;
  onPublicNoteChange: (value: string) => void;
  onInternalNoteChange: (value: string) => void;
}

function NoteTextarea({
  id,
  label,
  value,
  onChange,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useTranslation();
  const count = copyNoteLength(value);
  const atLimit = count >= COPY_NOTE_MAX_CHARS;
  const countId = `${id}-count`;

  return (
    <div>
      <label htmlFor={id} className={formLabelClass()}>
        {label}
      </label>
      <textarea
        id={id}
        value={value}
        rows={3}
        maxLength={COPY_NOTE_MAX_CHARS}
        onChange={(event) => onChange(clampCopyNote(event.target.value))}
        className={formTextareaClass()}
        aria-describedby={countId}
      />
      <p
        id={countId}
        className={`mt-1 text-right text-xs ${
          atLimit ? 'text-amber-700 dark:text-amber-400' : 'text-gray-500 dark:text-gray-400'
        }`}
      >
        {t('items.noteCharCount', { count, max: COPY_NOTE_MAX_CHARS })}
      </p>
    </div>
  );
}

/** Staff item form: public note and internal note, each capped at 4000 characters. */
export function CopyNoteFields({
  publicNote,
  internalNote,
  onPublicNoteChange,
  onInternalNoteChange,
}: CopyNoteFieldsProps) {
  const { t } = useTranslation();
  const baseId = useId();

  return (
    <div className="space-y-4">
      <NoteTextarea
        id={`${baseId}-public-note`}
        label={t('items.publicNote')}
        value={publicNote}
        onChange={onPublicNoteChange}
      />
      <NoteTextarea
        id={`${baseId}-internal-note`}
        label={t('items.internalNote')}
        value={internalNote}
        onChange={onInternalNoteChange}
      />
    </div>
  );
}

interface CopyNotesProps {
  audience: 'staff' | 'public';
  publicNote?: string | null;
  /** Ignored unless `audience` is `staff`. */
  internalNote?: string | null;
  compact?: boolean;
}

function noteText(value: string | null | undefined): string {
  return value?.trim() ?? '';
}

/**
 * Copy notes on an item record.
 * Public and OPAC views show `publicNote` only. `internalNote` is staff-only.
 */
export function CopyNotes({ audience, publicNote, internalNote, compact = false }: CopyNotesProps) {
  const { t } = useTranslation();
  const publicText = noteText(publicNote);
  const internalText = audience === 'staff' ? noteText(internalNote) : '';
  if (!publicText && !internalText) return null;

  const textClass = compact ? 'text-xs' : 'text-sm';

  return (
    <div className="mt-2 space-y-1">
      {publicText ? (
        <p className={`${textClass} text-gray-700 dark:text-gray-300 whitespace-pre-wrap break-words`}>
          <span className="text-gray-500 dark:text-gray-400">{t('items.publicNote')}: </span>
          {publicText}
        </p>
      ) : null}
      {internalText ? (
        <p className={`${textClass} text-gray-700 dark:text-gray-300 whitespace-pre-wrap break-words`}>
          <span className="text-gray-500 dark:text-gray-400">{t('items.internalNote')}: </span>
          {internalText}
        </p>
      ) : null}
    </div>
  );
}
