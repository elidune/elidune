import { useTranslation } from 'react-i18next';
import Input from '@/components/common/Input';
import { UnpaidThresholdFields } from '@/components/settings/UnpaidThresholdFields';
import { formatEuroAmount } from '@/utils/patronFines';
import type { GlobalUnpaidThresholdMode } from '@/types';

export type FinePolicyFieldsProps = {
  /** When false, late-fine activation was not loaded (admin config). */
  showToggle: boolean;
  /** 403 on the admin config read: this account cannot flip late fines. */
  showAdminNote?: boolean;
  switchDisabled?: boolean;
  enabled: boolean;
  dailyRate: string;
  blockMode: GlobalUnpaidThresholdMode;
  threshold: string;
  graceDays: number | null;
  maxAmount: string | null;
  hasMediaRules: boolean;
  dailyRateError?: string | null;
  thresholdError?: string | null;
  onEnabledChange: (enabled: boolean) => void;
  onDailyRateChange: (value: string) => void;
  onBlockModeChange: (mode: GlobalUnpaidThresholdMode) => void;
  onThresholdChange: (value: string) => void;
};

export function FinePolicyFields({
  showToggle,
  showAdminNote = false,
  switchDisabled = false,
  enabled,
  dailyRate,
  blockMode,
  threshold,
  graceDays,
  maxAmount,
  hasMediaRules,
  dailyRateError,
  thresholdError,
  onEnabledChange,
  onDailyRateChange,
  onBlockModeChange,
  onThresholdChange,
}: FinePolicyFieldsProps) {
  const { t, i18n } = useTranslation();
  const showAmounts = !showToggle || enabled;
  const blockingStillActive = showToggle && !enabled && blockMode === 'amount';

  return (
    <div className="max-w-md space-y-4">
      <p className="text-sm text-gray-500 dark:text-gray-400">{t('settings.finePolicy.help')}</p>
      {showToggle ? (
        <div className="flex items-center justify-between gap-3">
          <p id="fine-policy-enabled-label" className="text-sm font-medium text-gray-900 dark:text-white">
            {t('settings.finePolicy.enabled')}
          </p>
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            aria-labelledby="fine-policy-enabled-label"
            disabled={switchDisabled}
            onClick={() => onEnabledChange(!enabled)}
            className={`relative h-8 w-14 shrink-0 rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-800 disabled:cursor-not-allowed disabled:opacity-50 ${
              enabled ? 'bg-amber-500 dark:bg-amber-600' : 'bg-gray-300 dark:bg-gray-600'
            }`}
          >
            <span
              className={`pointer-events-none absolute top-1 left-1 block h-6 w-6 rounded-full bg-white shadow transition-transform duration-200 ease-out ${
                enabled ? 'translate-x-6' : 'translate-x-0'
              }`}
            />
          </button>
        </div>
      ) : showAdminNote ? (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('settings.finePolicy.adminOnly')}</p>
      ) : null}
      {switchDisabled && showToggle && (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('settings.finePolicy.notOverridable')}</p>
      )}
      {blockingStillActive && (
        <p className="text-sm text-amber-800 dark:text-amber-300" role="status">
          {t('settings.finePolicy.blockingStillActive')}
        </p>
      )}
      {showAmounts && (
        <div className="space-y-4">
          <Input
            id="fine-daily-rate"
            label={t('settings.finePolicy.dailyRate')}
            type="number"
            inputMode="decimal"
            min="0"
            step="0.01"
            required
            value={dailyRate}
            error={dailyRateError ?? undefined}
            hint={t('settings.finePolicy.dailyRateHint')}
            onChange={(e) => onDailyRateChange(e.target.value)}
          />
          {graceDays != null && (
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {t('settings.finePolicy.graceHint', { days: graceDays })}
            </p>
          )}
          {maxAmount && (
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {t('settings.finePolicy.capHint', {
                amount: formatEuroAmount(maxAmount, i18n.language),
              })}
            </p>
          )}
          {hasMediaRules && (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('settings.finePolicy.mediaRulesNote')}</p>
          )}
          <UnpaidThresholdFields
            scope="global"
            name="global-unpaid-threshold"
            mode={blockMode}
            amount={threshold}
            error={thresholdError}
            onModeChange={onBlockModeChange}
            onAmountChange={onThresholdChange}
          />
        </div>
      )}
    </div>
  );
}
