import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';
import { HOLD_NOTE_MAX_CHARS, clampHoldNote, holdNoteLength, holdNoteText } from '@/utils/holdNotes';

interface HoldNoteFieldsProps {
  patronNote: string;
  internalNote: string;
  onPatronNoteChange: (value: string) => void;
  onInternalNoteChange: (value: string) => void;
  /** Holds write. Readers and holds-read staff do not get the internal field. */
  canEditInternal: boolean;
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
  const count = holdNoteLength(value);
  const atLimit = count >= HOLD_NOTE_MAX_CHARS;
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
        onChange={(event) => onChange(clampHoldNote(event.target.value))}
        className={formTextareaClass()}
        aria-describedby={countId}
      />
      <p
        id={countId}
        className={`mt-1 text-right text-xs ${
          atLimit ? 'text-amber-700 dark:text-amber-400' : 'text-gray-500 dark:text-gray-400'
        }`}
      >
        {t('holds.noteCharCount', { count, max: HOLD_NOTE_MAX_CHARS })}
      </p>
    </div>
  );
}

/**
 * Place-hold notes.
 * `patronNote` is the reader's remark. `internalNote` is editable only with holds write.
 */
export function HoldNoteFields({
  patronNote,
  internalNote,
  onPatronNoteChange,
  onInternalNoteChange,
  canEditInternal,
}: HoldNoteFieldsProps) {
  const { t } = useTranslation();
  const baseId = useId();

  return (
    <div className="space-y-4">
      <NoteTextarea
        id={`${baseId}-patron-note`}
        label={t('holds.patronNote')}
        value={patronNote}
        onChange={onPatronNoteChange}
      />
      {canEditInternal ? (
        <NoteTextarea
          id={`${baseId}-internal-note`}
          label={t('holds.internalNote')}
          value={internalNote}
          onChange={onInternalNoteChange}
        />
      ) : null}
    </div>
  );
}

interface HoldNotesProps {
  audience: 'staff' | 'reader';
  patronNote?: string | null;
  /** Ignored unless `audience` is `staff`. */
  internalNote?: string | null;
  compact?: boolean;
}

/**
 * Hold notes on a queue row or on the reader's own list.
 * Search and public lists omit both fields: this renders nothing.
 * Closed and anonymized holds send null: this renders nothing.
 * Readers never see `internalNote`.
 */
export function HoldNotes({ audience, patronNote, internalNote, compact = false }: HoldNotesProps) {
  const { t } = useTranslation();
  const patronText = holdNoteText(patronNote);
  const internalText = audience === 'staff' ? holdNoteText(internalNote) : '';
  if (!patronText && !internalText) return null;

  const textClass = compact ? 'text-xs' : 'text-sm';

  return (
    <div className="mt-1 space-y-1">
      {patronText ? (
        <p className={`${textClass} text-gray-700 dark:text-gray-300 whitespace-pre-wrap break-words`}>
          <span className="text-gray-500 dark:text-gray-400">{t('holds.patronNote')}: </span>
          {patronText}
        </p>
      ) : null}
      {internalText ? (
        <p className={`${textClass} text-gray-700 dark:text-gray-300 whitespace-pre-wrap break-words`}>
          <span className="text-gray-500 dark:text-gray-400">{t('holds.internalNote')}: </span>
          {internalText}
        </p>
      ) : null}
    </div>
  );
}
