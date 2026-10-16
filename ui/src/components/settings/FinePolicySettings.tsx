import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Save } from 'lucide-react';
import { Card, CardHeader, Button } from '@/components/common';
import { FinePolicyFields } from '@/components/settings/FinePolicyFields';
import { useToast } from '@/contexts/ToastContext';
import {
  useFinePolicyQuery,
  useFineRulesQuery,
  useUpdateFinePolicyMutation,
  useUpsertFineRuleMutation,
} from '@/hooks/settings/useFinePolicyQuery';
import {
  useRemindersConfigQuery,
  useUpdateRemindersConfigMutation,
} from '@/hooks/settings/useRemindersConfigQuery';
import type { CirculationFinePolicy, FineRule, GlobalUnpaidThresholdMode } from '@/types';
import { getApiErrorMessage, getHttpStatus } from '@/utils/apiError';
import { fineSettingsDraft, fineSettingsSavePlan } from '@/utils/finePolicy';
import { withAccrueFines } from '@/utils/reminderPolicy';

export default function FinePolicySettings() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const policyQuery = useFinePolicyQuery();
  const rulesQuery = useFineRulesQuery();
  const remindersQuery = useRemindersConfigQuery();
  const updatePolicy = useUpdateFinePolicyMutation();
  const upsertRule = useUpsertFineRuleMutation();
  const updateReminders = useUpdateRemindersConfigMutation();

  const accrualKnown = remindersQuery.isSuccess;
  // GET /admin/config is admin-only, same gate as the reminders PUT. A failed read means
  // this session cannot turn late-fine accrual on or off; amount and threshold still save.
  const canControlAccrual = Boolean(accrualKnown && remindersQuery.data?.overridable);
  const accrueFines = remindersQuery.data?.config.accrueFines ?? true;

  const [enabled, setEnabled] = useState(true);
  const [dailyRate, setDailyRate] = useState('');
  const [blockMode, setBlockMode] = useState<GlobalUnpaidThresholdMode>('none');
  const [threshold, setThreshold] = useState('');
  const [graceDays, setGraceDays] = useState<number | null>(null);
  const [maxAmount, setMaxAmount] = useState<string | null>(null);
  const [hasMediaRules, setHasMediaRules] = useState(false);
  const [dailyRateError, setDailyRateError] = useState<string | null>(null);
  const [thresholdError, setThresholdError] = useState<string | null>(null);
  const [synced, setSynced] = useState<{
    policy?: CirculationFinePolicy;
    rules?: FineRule[];
    accrue?: boolean;
    known: boolean;
  }>({ known: false });

  const policy = policyQuery.data;
  const rules = rulesQuery.data;
  if (
    policy !== synced.policy ||
    rules !== synced.rules ||
    accrueFines !== synced.accrue ||
    accrualKnown !== synced.known
  ) {
    setSynced({ policy, rules, accrue: accrueFines, known: accrualKnown });
    if (policy && rules) {
      const draft = fineSettingsDraft(policy, accrualKnown ? accrueFines : true, rules);
      setEnabled(accrualKnown ? draft.enabled : true);
      setDailyRate(draft.dailyRate);
      setBlockMode(draft.blockMode);
      setThreshold(draft.threshold);
      setGraceDays(draft.graceDays);
      setMaxAmount(draft.maxAmount);
      setHasMediaRules(draft.hasMediaRules);
      setDailyRateError(null);
      setThresholdError(null);
    }
  }

  const coreLoading = policyQuery.isLoading || rulesQuery.isLoading || remindersQuery.isLoading;
  const coreError = policyQuery.isError || rulesQuery.isError;
  const ready = policy != null && rules != null && !coreLoading && !coreError;
  const saving = updatePolicy.isPending || upsertRule.isPending || updateReminders.isPending;
  const showToggleDisabled = accrualKnown && !canControlAccrual;
  const saveDisabled = !ready || saving || (!canControlAccrual && !enabled);

  const handleSave = async () => {
    if (!policy || !rules) return;
    const plan = fineSettingsSavePlan({
      enabled: accrualKnown ? enabled : true,
      canControlAccrual,
      dailyRate,
      blockMode,
      threshold,
      rules,
    });
    if (!plan.ok) {
      setDailyRateError(plan.field === 'dailyRate' ? t('settings.finePolicy.invalidAmount') : null);
      setThresholdError(plan.field === 'threshold' ? t('settings.finePolicy.invalidAmount') : null);
      return;
    }
    setDailyRateError(null);
    setThresholdError(null);
    const remindersPayload =
      plan.accrueFines == null || !remindersQuery.data
        ? null
        : withAccrueFines(remindersQuery.data.raw, plan.accrueFines);
    if (plan.accrueFines != null && remindersPayload == null) {
      showToast({ variant: 'error', message: t('settings.finePolicy.accrualSaveError') });
      return;
    }
    try {
      if (plan.rule) await upsertRule.mutateAsync(plan.rule);
      if (plan.policy) await updatePolicy.mutateAsync(plan.policy);
      if (remindersPayload) await updateReminders.mutateAsync(remindersPayload);
      showToast({ variant: 'success', message: t('settings.finePolicy.saveSuccess') });
    } catch (err: unknown) {
      showToast({ variant: 'error', message: getApiErrorMessage(err, t) });
    }
  };

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader
        title={t('settings.finePolicy.title')}
        action={
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              void handleSave();
            }}
            isLoading={saving}
            disabled={saveDisabled}
            leftIcon={<Save className="h-4 w-4" />}
          >
            {t('common.save')}
          </Button>
        }
      />
      <div className="px-4 pb-6 sm:px-6 space-y-3">
        {coreLoading && (
          <div className="flex items-center justify-center h-16">
            <div className="h-6 w-6 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
          </div>
        )}
        {coreError && (
          <div
            className="flex items-center gap-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 px-3 py-2 text-sm text-red-700 dark:text-red-400"
            role="alert"
          >
            <span>{t('settings.finePolicy.loadError')}</span>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => {
                void policyQuery.refetch();
                void rulesQuery.refetch();
              }}
            >
              {t('common.retry')}
            </Button>
          </div>
        )}
        {ready && remindersQuery.isError && getHttpStatus(remindersQuery.error) !== 403 && (
          <div
            className="flex items-center gap-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 px-3 py-2 text-sm text-red-700 dark:text-red-400"
            role="alert"
          >
            <span>{t('settings.finePolicy.accrualLoadError')}</span>
            <Button size="sm" variant="ghost" onClick={() => void remindersQuery.refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        {ready && (
          <FinePolicyFields
            showToggle={accrualKnown}
            showAdminNote={!accrualKnown && getHttpStatus(remindersQuery.error) === 403}
            switchDisabled={showToggleDisabled}
            enabled={enabled}
            dailyRate={dailyRate}
            blockMode={blockMode}
            threshold={threshold}
            graceDays={graceDays}
            maxAmount={maxAmount}
            hasMediaRules={hasMediaRules}
            dailyRateError={dailyRateError}
            thresholdError={thresholdError}
            onEnabledChange={(next) => {
              setEnabled(next);
              setDailyRateError(null);
              setThresholdError(null);
            }}
            onDailyRateChange={(value) => {
              setDailyRate(value);
              if (dailyRateError) setDailyRateError(null);
            }}
            onBlockModeChange={(mode) => {
              setBlockMode(mode);
              if (thresholdError) setThresholdError(null);
            }}
            onThresholdChange={(value) => {
              setThreshold(value);
              if (thresholdError) setThresholdError(null);
            }}
          />
        )}
      </div>
    </Card>
  );
}
