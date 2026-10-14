import { useId, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';

interface CirculationOverrideReasonFieldProps {
  value: string;
  onChange: (value: string) => void;
  error?: string | null;
  disabled?: boolean;
  inputRef?: Ref<HTMLTextAreaElement>;
}

/** « Motif du forçage ». The error is tied to the textarea with `aria-describedby`. */
export default function CirculationOverrideReasonField({
  value,
  onChange,
  error,
  disabled = false,
  inputRef,
}: CirculationOverrideReasonFieldProps) {
  const { t } = useTranslation();
  const inputId = useId();
  const errorId = useId();
  const describedBy = error ? errorId : undefined;

  return (
    <div className="mt-4">
      <label htmlFor={inputId} className={formLabelClass()}>
        {t('loans.forceOverrideReason')}
      </label>
      <textarea
        ref={inputRef}
        id={inputId}
        className={formTextareaClass({ error: Boolean(error) })}
        rows={3}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        disabled={disabled}
        autoFocus
        aria-required="true"
        aria-invalid={error ? true : undefined}
        aria-describedby={describedBy}
      />
      {error ? (
        <p id={errorId} role="alert" className="mt-1 text-sm text-red-600 dark:text-red-400 whitespace-pre-wrap">
          {error}
        </p>
      ) : null}
    </div>
  );
}
