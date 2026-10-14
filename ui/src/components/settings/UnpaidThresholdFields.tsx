import type { ReactNode } from 'react';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import Input from '@/components/common/Input';
import { formChoiceControlClass, formChoiceLabelClass, formLabelClass } from '@/utils/formControl';
import { formatEuroAmount } from '@/utils/patronFines';
import type { AudienceUnpaidThresholdMode, GlobalUnpaidThresholdMode } from '@/types';

type ScopeProps =
  | {
      scope: 'global';
      mode: GlobalUnpaidThresholdMode;
      onModeChange: (mode: GlobalUnpaidThresholdMode) => void;
      inheritedAmount?: never;
    }
  | {
      scope: 'audience';
      mode: AudienceUnpaidThresholdMode;
      onModeChange: (mode: AudienceUnpaidThresholdMode) => void;
      /** Euro decimal of the global amount (`12.50`). Null when the global mode does not block. */
      inheritedAmount?: string | null;
      /** False when scheduled late fines are off. Stored thresholds stay visible and are ignored. */
      globalFinesEnabled?: boolean;
    };

export type UnpaidThresholdFieldsProps = ScopeProps & {
  /** Shared radio name. Must be unique per form on the page. */
  name: string;
  amount: string;
  onAmountChange: (value: string) => void;
  error?: string | null;
  legendEnd?: ReactNode;
};

const GLOBAL_MODES = ['none', 'amount'] as const;
const AUDIENCE_MODES = ['inherit', 'none', 'amount'] as const;

function modeLabel(
  scope: 'global' | 'audience',
  mode: AudienceUnpaidThresholdMode,
  t: (key: string) => string,
): string {
  if (scope === 'global') {
    return mode === 'amount' ? t('settings.finePolicy.modeAmount') : t('settings.finePolicy.modeNone');
  }
  if (mode === 'inherit') return t('settings.publicTypes.unpaidThresholdModeInherit');
  if (mode === 'amount') return t('settings.publicTypes.unpaidThresholdModeAmount');
  return t('settings.publicTypes.unpaidThresholdModeNone');
}

export function UnpaidThresholdFields(props: UnpaidThresholdFieldsProps) {
  const { t, i18n } = useTranslation();
  const hintId = useId();
  const inactiveHintId = useId();
  const finesOff = props.scope === 'audience' && props.globalFinesEnabled === false;
  const helpId = useId();
  const modes = props.scope === 'global' ? GLOBAL_MODES : AUDIENCE_MODES;
  const title =
    props.scope === 'global'
      ? t('settings.finePolicy.mode')
      : t('settings.publicTypes.unpaidThresholdMode');
  const amountLabel =
    props.scope === 'global'
      ? t('settings.finePolicy.unpaidFineThreshold')
      : t('settings.publicTypes.unpaidFineThreshold');

  const selectMode = (value: (typeof AUDIENCE_MODES)[number]) => {
    if (props.scope === 'global') {
      props.onModeChange(value === 'amount' ? 'amount' : 'none');
      return;
    }
    props.onModeChange(value);
  };

  const describedBy =
    props.scope === 'audience' ? (finesOff ? inactiveHintId : helpId) : undefined;

  return (
    <div className="flex items-start justify-between gap-3">
      <div className="min-w-0 flex-1 space-y-2">
        {finesOff && (
          <p id={inactiveHintId} className="text-xs text-gray-500 dark:text-gray-400">
            {t('settings.publicTypes.finesDisabledNoEffect')}
          </p>
        )}
        <fieldset
          className={`space-y-2 border-0 p-0 ${finesOff ? 'opacity-60' : ''}`}
          disabled={finesOff}
          aria-describedby={describedBy}
        >
          <legend className={formLabelClass({ marginBottom: false })}>{title}</legend>
          <div className="flex flex-col gap-2">
            {modes.map((value) => (
              <label key={value} className={formChoiceLabelClass()}>
                <input
                  type="radio"
                  name={props.name}
                  value={value}
                  checked={props.mode === value}
                  disabled={finesOff}
                  className={formChoiceControlClass('rounded-full')}
                  aria-describedby={
                    value === 'inherit' && props.mode === 'inherit' && !finesOff ? hintId : undefined
                  }
                  onChange={() => selectMode(value)}
                />
                {modeLabel(props.scope, value, t)}
              </label>
            ))}
          </div>
          {props.scope === 'audience' && props.mode === 'inherit' && !finesOff && (
            <p id={hintId} className="text-xs text-gray-500 dark:text-gray-400">
              {props.inheritedAmount
                ? t('settings.publicTypes.unpaidFineThresholdInherit', {
                    amount: formatEuroAmount(props.inheritedAmount, i18n.language),
                  })
                : t('settings.publicTypes.unpaidFineThresholdInheritEmpty')}
            </p>
          )}
          {props.mode === 'amount' && (
            <Input
              label={amountLabel}
              type="number"
              inputMode="decimal"
              min="0"
              step="0.01"
              required
              disabled={finesOff}
              value={props.amount}
              error={props.error ?? undefined}
              hint={finesOff ? undefined : t('settings.finePolicy.thresholdHint')}
              onChange={(e) => props.onAmountChange(e.target.value)}
            />
          )}
          {props.scope === 'audience' && !finesOff && (
            <p id={helpId} className="text-xs text-gray-500 dark:text-gray-400">
              {t('settings.publicTypes.unpaidFineThresholdHelp')}
            </p>
          )}
        </fieldset>
      </div>
      {props.legendEnd}
    </div>
  );
}
