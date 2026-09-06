import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Save } from 'lucide-react';
import { Card, CardHeader, Button, Input } from '@/components/common';
import { useToast } from '@/contexts/ToastContext';
import {
  useHoldsPolicyQuery,
  useUpdateHoldsPolicyMutation,
} from '@/hooks/settings/useHoldsPolicyQuery';
import { getApiErrorMessage } from '@/utils/apiError';
import { holdCapToInput, parseHoldCapInput } from '@/utils/holdsPolicy';
import type { HoldsPolicy } from '@/types';

export default function HoldsPolicySettings() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const { data, isLoading, isError, refetch } = useHoldsPolicyQuery();
  const updatePolicy = useUpdateHoldsPolicyMutation();
  const [draft, setDraft] = useState('20');
  const [fieldError, setFieldError] = useState<string | null>(null);
  const [syncedPolicy, setSyncedPolicy] = useState<HoldsPolicy | undefined>(data);
  if (data !== syncedPolicy) {
    setSyncedPolicy(data);
    if (data) {
      setDraft(holdCapToInput(data.maxActiveHolds) || '20');
      setFieldError(null);
    }
  }

  const handleSave = async () => {
    const parsed = parseHoldCapInput(draft, false);
    if (!parsed.ok || parsed.value == null) {
      setFieldError(t('settings.holdsPolicy.invalidCap'));
      return;
    }
    setFieldError(null);
    try {
      const next = await updatePolicy.mutateAsync({ maxActiveHolds: parsed.value });
      setDraft(holdCapToInput(next.maxActiveHolds) || String(parsed.value));
      showToast({ variant: 'success', message: t('settings.holdsPolicy.saveSuccess') });
    } catch (err: unknown) {
      showToast({ variant: 'error', message: getApiErrorMessage(err, t) });
    }
  };

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader
        title={t('settings.holdsPolicy.title')}
        action={
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              void handleSave();
            }}
            isLoading={updatePolicy.isPending}
            disabled={isLoading || isError}
            leftIcon={<Save className="h-4 w-4" />}
          >
            {t('common.save')}
          </Button>
        }
      />
      <div className="px-4 pb-6 sm:px-6 space-y-3">
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('settings.holdsPolicy.help')}</p>
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
            <span>{t('settings.holdsPolicy.loadError')}</span>
            <Button size="sm" variant="ghost" onClick={() => void refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        {!isLoading && !isError && (
          <div className="max-w-xs">
            <Input
              label={t('settings.holdsPolicy.maxActiveHolds')}
              type="number"
              inputMode="numeric"
              min="1"
              max="1000"
              step="1"
              value={draft}
              error={fieldError ?? undefined}
              hint={t('settings.holdsPolicy.capHint')}
              onChange={(e) => {
                setDraft(e.target.value);
                if (fieldError) setFieldError(null);
              }}
            />
          </div>
        )}
      </div>
    </Card>
  );
}
