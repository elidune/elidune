import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Save } from 'lucide-react';
import { Card, CardHeader, Button } from '@/components/common';
import { useToast } from '@/contexts/ToastContext';
import {
  useRemindersConfigQuery,
  useUpdateRemindersConfigMutation,
} from '@/hooks/settings/useRemindersConfigQuery';
import ReminderTiersFields from '@/components/settings/ReminderTiersFields';
import { getApiErrorMessage } from '@/utils/apiError';
import {
  applyDraftToRemindersConfig,
  mergeReminderTiersIntoPayload,
  parseReminderTiersDraft,
  remindersConfigToDraft,
  type ReminderTiersDraft,
} from '@/utils/reminderPolicy';
import type { RemindersConfig } from '@/types';

export default function OverdueRemindersSettings() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const { data, isLoading, isError, refetch } = useRemindersConfigQuery();
  const updateConfig = useUpdateRemindersConfigMutation();
  const [draft, setDraft] = useState<ReminderTiersDraft | null>(null);
  const [fieldError, setFieldError] = useState<'delay' | 'template' | null>(null);
  const [syncedConfig, setSyncedConfig] = useState<RemindersConfig | undefined>(data?.config);

  if (data?.config !== syncedConfig) {
    setSyncedConfig(data?.config);
    if (data?.config) {
      setDraft(remindersConfigToDraft(data.config));
      setFieldError(null);
    }
  }

  const handleSave = async () => {
    if (!data || !draft) return;
    const parsed = parseReminderTiersDraft(draft);
    if (!parsed.ok) {
      setFieldError(parsed.field);
      return;
    }
    const next = applyDraftToRemindersConfig(data.config, draft);
    if (!next) {
      setFieldError('delay');
      return;
    }
    setFieldError(null);
    try {
      await updateConfig.mutateAsync(mergeReminderTiersIntoPayload(data.raw, next));
      setDraft(remindersConfigToDraft(next));
      showToast({ variant: 'success', message: t('settings.overdueReminders.saveSuccess') });
    } catch (err: unknown) {
      showToast({ variant: 'error', message: getApiErrorMessage(err, t) });
    }
  };

  const canEdit = Boolean(data?.overridable);

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader
        title={t('settings.overdueReminders.title')}
        action={
          <Button
            size="sm"
            variant="primary"
            onClick={() => {
              void handleSave();
            }}
            isLoading={updateConfig.isPending}
            disabled={isLoading || isError || !canEdit}
            leftIcon={<Save className="h-4 w-4" />}
          >
            {t('common.save')}
          </Button>
        }
      />
      <div className="px-4 pb-6 sm:px-6 space-y-3">
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('settings.overdueReminders.help')}</p>
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
            <span>{t('settings.overdueReminders.loadError')}</span>
            <Button size="sm" variant="ghost" onClick={() => void refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        {!isLoading && !isError && data && !canEdit && (
          <p className="text-xs text-gray-500 dark:text-gray-400">{t('settings.overdueReminders.notOverridable')}</p>
        )}
        {!isLoading && !isError && draft && (
          <ReminderTiersFields
            draft={draft}
            onChange={(next) => {
              setDraft(next);
              if (fieldError) setFieldError(null);
            }}
            disabled={!canEdit}
            delayError={fieldError === 'delay'}
            templateError={fieldError === 'template'}
          />
        )}
      </div>
    </Card>
  );
}
