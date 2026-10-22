import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RotateCcw, Save } from 'lucide-react';
import { Card, CardHeader, Button } from '@/components/common';
import { useToast } from '@/contexts/ToastContext';
import {
  FEATURE_KEYS,
  parseFeaturesConfig,
  useFeaturesConfigQuery,
  useResetFeaturesConfigMutation,
  useUpdateFeaturesConfigMutation,
  type FeatureConfigKey,
} from '@/hooks/settings/useFeaturesConfigQuery';
import { getApiErrorMessage } from '@/utils/apiError';
import { formChoiceControlClass, formChoiceLabelClass } from '@/utils/formControl';
import type { FeaturesConfig } from '@/types';

function configToDraft(config: FeaturesConfig): Record<FeatureConfigKey, boolean> {
  return {
    acquisitions: config.acquisitions,
    purchase_suggestions: config.purchase_suggestions,
    weeding: config.weeding,
    transits: config.transits,
    deposits: config.deposits,
    events: config.events,
    inventory: config.inventory,
    z3950: config.z3950,
    import_iso: config.import_iso,
    stats: config.stats,
    visitor_counts: config.visitor_counts,
  };
}

export default function ModulesSettings() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const { data, isLoading, isError, refetch } = useFeaturesConfigQuery();
  const updateConfig = useUpdateFeaturesConfigMutation();
  const resetConfig = useResetFeaturesConfigMutation();
  const [draft, setDraft] = useState<Record<FeatureConfigKey, boolean> | null>(null);
  const [syncedRaw, setSyncedRaw] = useState<string | null>(null);

  const rawKey = data ? JSON.stringify(data.raw) : null;
  if (rawKey !== syncedRaw) {
    setSyncedRaw(rawKey);
    if (data?.config) {
      setDraft(configToDraft(data.config));
    }
  }

  const handleSave = async () => {
    if (!data || !draft) return;
    try {
      const payload: Record<string, unknown> = {
        ...data.raw,
        ...draft,
        overridable: data.config.overridable ?? true,
      };
      await updateConfig.mutateAsync(payload);
      setDraft(configToDraft(parseFeaturesConfig(payload)));
      showToast({ variant: 'success', message: t('settings.modules.saveSuccess') });
    } catch (err: unknown) {
      showToast({ variant: 'error', message: getApiErrorMessage(err, t) });
    }
  };

  const handleReset = async () => {
    try {
      await resetConfig.mutateAsync();
      showToast({ variant: 'success', message: t('settings.modules.resetSuccess') });
    } catch (err: unknown) {
      showToast({ variant: 'error', message: getApiErrorMessage(err, t) });
    }
  };

  const canEdit = Boolean(data?.overridable);
  const busy = updateConfig.isPending || resetConfig.isPending;

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader
        title={t('settings.modules.title')}
        action={
          <div className="flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="ghost"
              leftIcon={<RotateCcw className="h-4 w-4" />}
              onClick={() => {
                void handleReset();
              }}
              isLoading={resetConfig.isPending}
              disabled={isLoading || isError || !canEdit || !data?.overridden}
            >
              {t('settings.modules.resetToFile')}
            </Button>
            <Button
              size="sm"
              variant="primary"
              leftIcon={<Save className="h-4 w-4" />}
              onClick={() => {
                void handleSave();
              }}
              isLoading={updateConfig.isPending}
              disabled={isLoading || isError || !canEdit || !data || !draft || busy}
            >
              {t('common.save')}
            </Button>
          </div>
        }
      />
      <div className="px-4 pb-6 sm:px-6 space-y-4">
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('settings.modules.help')}</p>
        {data?.overridden ? (
          <p className="text-xs text-amber-700 dark:text-amber-300">{t('settings.modules.overriddenHint')}</p>
        ) : null}
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
            <span>{t('settings.modules.loadError')}</span>
            <Button size="sm" variant="ghost" onClick={() => void refetch()}>
              {t('common.retry')}
            </Button>
          </div>
        )}
        {draft && (
          <ul className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            {FEATURE_KEYS.map((key) => (
              <li key={key}>
                <label className={formChoiceLabelClass()}>
                  <input
                    type="checkbox"
                    checked={draft[key]}
                    disabled={!canEdit}
                    onChange={(e) => setDraft((prev) => (prev ? { ...prev, [key]: e.target.checked } : prev))}
                    className={formChoiceControlClass('rounded')}
                  />
                  <span>
                    <span className="font-medium text-gray-900 dark:text-gray-100">
                      {t(`settings.modules.flags.${key}`)}
                    </span>
                    <span className="block text-xs text-gray-500 dark:text-gray-400">
                      {t(`settings.modules.flagHelp.${key}`)}
                    </span>
                  </span>
                </label>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Card>
  );
}
