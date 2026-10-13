import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Save } from 'lucide-react';
import { Card, CardHeader, Button } from '@/components/common';
import { useToast } from '@/contexts/ToastContext';
import { useFinePolicyQuery, useUpdateFinePolicyMutation } from '@/hooks/settings/useFinePolicyQuery';
import { getApiErrorMessage } from '@/utils/apiError';
import { centsToInput, globalUnpaidThresholdBody } from '@/utils/finePolicy';
import type { CirculationFinePolicy, GlobalUnpaidThresholdMode } from '@/types';
import { UnpaidThresholdFields } from '@/components/settings/UnpaidThresholdFields';

export default function FinePolicySettings() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const { data, isLoading, isError, refetch } = useFinePolicyQuery();
  const updatePolicy = useUpdateFinePolicyMutation();
  const [mode, setMode] = useState<GlobalUnpaidThresholdMode>(data?.unpaidThresholdMode ?? 'none');
  const [draft, setDraft] = useState(() => centsToInput(data?.unpaidThresholdCents));
  const [fieldError, setFieldError] = useState<string | null>(null);
  // Start unsynced so a cached policy (already loaded by the settings shell) is copied
  // into the draft. Initialising synced state to `data` skips that copy.
  const [syncedPolicy, setSyncedPolicy] = useState<CirculationFinePolicy | undefined>(undefined);
  if (data !== syncedPolicy) {
    setSyncedPolicy(data);
    if (data) {
      setMode(data.unpaidThresholdMode);
      setDraft(centsToInput(data.unpaidThresholdCents));
      setFieldError(null);
    }
  }

  const handleSave = async () => {
    if (!data) return;
    const built = globalUnpaidThresholdBody(mode, draft);
    if (!built.ok) {
      setFieldError(t('settings.finePolicy.invalidAmount'));
      return;
    }
    setFieldError(null);
    try {
      const next = await updatePolicy.mutateAsync(built.body);
      setMode(next.unpaidThresholdMode);
      setDraft(centsToInput(next.unpaidThresholdCents));
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
            isLoading={updatePolicy.isPending}
            disabled={isLoading || isError || data == null}
            leftIcon={<Save className="h-4 w-4" />}
          >
            {t('common.save')}
          </Button>
        }
      />
      <div className="px-4 pb-6 sm:px-6 space-y-3">
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('settings.finePolicy.help')}</p>
        {isLoading && (
          <div className="flex items-center justify-center h-16">
            <div className="h-6 w-6 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
          </div>
        )}
        {isError && (
          <div
            className="flex items-center gap-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 px-3 py-2 text-sm text-red-700 dark:text-red-400"
            role="alert"
          >
            <span>{t('settings.finePolicy.loadError')}</span>
            <Button size="sm" variant="ghost" onClick={() => void refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        {!isLoading && !isError && (
          <div className="max-w-md">
            <UnpaidThresholdFields
              scope="global"
              name="global-unpaid-threshold"
              mode={mode}
              amount={draft}
              error={fieldError}
              onModeChange={(next) => {
                setMode(next);
                setFieldError(null);
              }}
              onAmountChange={(value) => {
                setDraft(value);
                if (fieldError) setFieldError(null);
              }}
            />
          </div>
        )}
      </div>
    </Card>
  );
}
