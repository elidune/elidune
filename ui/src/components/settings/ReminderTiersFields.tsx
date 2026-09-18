import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Input } from '@/components/common';
import api from '@/services/api';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { EMAIL_TEMPLATES_LIST_QUERY_KEY, filterEmailTemplatesBySupportedLanguages } from '@/utils/emailTemplatesConstants';
import {
  MAX_REMINDER_DELAY_DAYS,
  MIN_REMINDER_DELAY_DAYS,
  REMINDER_TIER_ORDER,
  applyReminderTierEnabled,
  emailTemplateSelectOptions,
  isReminderTierSwitchDisabled,
  normalizeReminderTiersDraft,
  parseReminderDelayDays,
  type ReminderTiersDraft,
} from '@/utils/reminderPolicy';
import { fromI18nLanguage } from '@/locales';
import type { ReminderTierId } from '@/types';

type ReminderTiersFieldsProps = {
  draft: ReminderTiersDraft;
  onChange: (next: ReminderTiersDraft) => void;
  disabled?: boolean;
  delayError?: boolean;
  templateError?: boolean;
};

const TIER_LABEL_KEY: Record<ReminderTierId, string> = {
  first: 'settings.overdueReminders.first',
  second: 'settings.overdueReminders.second',
  formalNotice: 'settings.overdueReminders.formalNotice',
};

function enabledKey(tier: ReminderTierId): keyof ReminderTiersDraft {
  if (tier === 'first') return 'firstEnabled';
  if (tier === 'second') return 'secondEnabled';
  return 'formalNoticeEnabled';
}

function delayKey(tier: ReminderTierId): keyof ReminderTiersDraft {
  if (tier === 'first') return 'firstDelayDays';
  if (tier === 'second') return 'secondDelayDays';
  return 'formalNoticeDelayDays';
}

function templateKey(tier: ReminderTierId): keyof ReminderTiersDraft {
  if (tier === 'first') return 'firstTemplate';
  if (tier === 'second') return 'secondTemplate';
  return 'formalNoticeTemplate';
}

export default function ReminderTiersFields({
  draft,
  onChange,
  disabled = false,
  delayError = false,
  templateError = false,
}: ReminderTiersFieldsProps) {
  const { t, i18n } = useTranslation();
  const uiLanguage = fromI18nLanguage(i18n.language);

  const { data: listRaw = [] } = useQuery({
    queryKey: EMAIL_TEMPLATES_LIST_QUERY_KEY,
    queryFn: () => api.getEmailTemplates(),
  });

  const options = useMemo(
    () =>
      emailTemplateSelectOptions(filterEmailTemplatesBySupportedLanguages(listRaw), uiLanguage, [
        draft.firstTemplate,
        draft.secondTemplate,
        draft.formalNoticeTemplate,
      ]),
    [listRaw, uiLanguage, draft.firstTemplate, draft.secondTemplate, draft.formalNoticeTemplate],
  );

  const emit = (next: ReminderTiersDraft) => onChange(normalizeReminderTiersDraft(next));

  return (
    <div className="space-y-3">
      <ul className="space-y-3">
        {REMINDER_TIER_ORDER.map((tier) => {
          const on = Boolean(draft[enabledKey(tier)]);
          const switchDisabled = disabled || isReminderTierSwitchDisabled(draft, tier);
          const delayId = `reminder-tier-delay-${tier}`;
          const templateId = `reminder-tier-template-${tier}`;
          return (
            <li
              key={tier}
              className="rounded-xl border border-gray-200 dark:border-gray-700 bg-gray-50/70 dark:bg-gray-900/40 px-3 py-3 sm:px-4"
            >
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-gray-900 dark:text-white">
                    {t(TIER_LABEL_KEY[tier])}
                  </p>
                  {tier === 'formalNotice' && (
                    <p className="mt-0.5 text-xs text-gray-500 dark:text-gray-400">
                      {t('settings.overdueReminders.formalNoticeHint')}
                    </p>
                  )}
                </div>
                <button
                  type="button"
                  role="switch"
                  aria-checked={on}
                  aria-label={t(TIER_LABEL_KEY[tier])}
                  disabled={switchDisabled}
                  onClick={() => emit(applyReminderTierEnabled(draft, tier, !on))}
                  className={`relative h-8 w-14 shrink-0 rounded-full transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-gray-800 disabled:cursor-not-allowed disabled:opacity-50 ${
                    on ? 'bg-amber-500 dark:bg-amber-600' : 'bg-gray-300 dark:bg-gray-600'
                  }`}
                >
                  <span
                    className={`pointer-events-none absolute top-1 left-1 block h-6 w-6 rounded-full bg-white shadow transition-transform duration-200 ease-out ${
                      on ? 'translate-x-6' : 'translate-x-0'
                    }`}
                  />
                </button>
              </div>
              <div className="mt-3 grid grid-cols-1 sm:grid-cols-2 gap-3">
                <Input
                  id={delayId}
                  label={t('settings.overdueReminders.delayDays')}
                  type="number"
                  inputMode="numeric"
                  min={MIN_REMINDER_DELAY_DAYS}
                  max={MAX_REMINDER_DELAY_DAYS}
                  step={1}
                  value={String(draft[delayKey(tier)])}
                  error={
                    delayError && !parseReminderDelayDays(String(draft[delayKey(tier)])).ok
                      ? t('settings.overdueReminders.invalidDelay')
                      : undefined
                  }
                  disabled={disabled}
                  onChange={(e) => emit({ ...draft, [delayKey(tier)]: e.target.value })}
                />
                <div className="w-full">
                  <label htmlFor={templateId} className={formLabelClass()}>
                    {t('settings.overdueReminders.template')}
                  </label>
                  <select
                    id={templateId}
                    value={String(draft[templateKey(tier)])}
                    disabled={disabled}
                    aria-invalid={
                      templateError && !String(draft[templateKey(tier)]).trim() ? 'true' : undefined
                    }
                    onChange={(e) => emit({ ...draft, [templateKey(tier)]: e.target.value })}
                    className={formControlClass({
                      error: Boolean(templateError && !String(draft[templateKey(tier)]).trim()),
                      className: 'w-full',
                    })}
                  >
                    {options.map((opt) => (
                      <option key={opt.id} value={opt.id}>
                        {opt.label}
                      </option>
                    ))}
                  </select>
                  {templateError && !String(draft[templateKey(tier)]).trim() && (
                    <p className="mt-1 text-sm text-red-600 dark:text-red-400">
                      {t('settings.overdueReminders.invalidTemplate')}
                    </p>
                  )}
                </div>
              </div>
            </li>
          );
        })}
      </ul>
      <p className="text-xs text-gray-500 dark:text-gray-400">{t('settings.overdueReminders.chainHint')}</p>
    </div>
  );
}
