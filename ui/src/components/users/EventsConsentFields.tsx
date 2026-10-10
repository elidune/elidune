import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { formChoiceLabelClass } from '@/utils/formControl';
import { eventsConsentStatusText } from '@/utils/eventsConsent';

export interface EventsConsentFieldsProps {
  /** Unique prefix so radio names do not collide across forms. */
  name: string;
  /** Current choice. Create forms start at false (No). */
  value: boolean;
  onChange: (next: boolean) => void;
  /** Child accounts have no consent of their own. */
  isChild: boolean;
  guardianId?: string | null;
  /** Staff patron file can open the guardian. Hidden when the viewer cannot. */
  showGuardianLink?: boolean;
  /** Saved consent, shown on edit and profile. Omitted on creation. */
  recorded?: {
    consent?: boolean | null;
    at?: string | null;
    source?: string | null;
  } | null;
}

export default function EventsConsentFields({
  name,
  value,
  onChange,
  isChild,
  guardianId,
  showGuardianLink = false,
  recorded = null,
}: EventsConsentFieldsProps) {
  const { t, i18n } = useTranslation();
  const hintId = `${name}-hint`;
  const statusId = `${name}-status`;
  const status = recorded
    ? eventsConsentStatusText(
        t,
        i18n.language,
        recorded.consent,
        recorded.at,
        recorded.source
      )
    : null;
  const guardianHref =
    showGuardianLink && guardianId ? `/users/${guardianId}` : null;

  return (
    <div className="rounded-lg border border-amber-200 bg-amber-50/80 p-3 dark:border-amber-800/60 dark:bg-amber-950/30">
      {isChild ? (
        <div className="space-y-1.5">
          <p className="text-sm font-medium text-gray-900 dark:text-gray-100">
            {t('eventsConsent.legend')}
          </p>
          <p className="text-sm text-gray-700 dark:text-gray-300">{t('eventsConsent.childNotice')}</p>
          {guardianHref ? (
            <Link
              to={guardianHref}
              className="inline-flex text-sm font-medium text-indigo-600 hover:underline dark:text-indigo-400"
            >
              {t('eventsConsent.openGuardianFile')}
            </Link>
          ) : null}
        </div>
      ) : (
        <fieldset aria-describedby={status ? `${hintId} ${statusId}` : hintId}>
          <legend className="text-sm font-medium text-gray-900 dark:text-gray-100">
            {t('eventsConsent.legend')}
          </legend>
          <div className="mt-2 flex flex-wrap gap-4">
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name={name}
                value="yes"
                className="text-indigo-600 focus-visible:ring-2 focus-visible:ring-amber-500"
                checked={value === true}
                onChange={() => onChange(true)}
              />
              {t('eventsConsent.yes')}
            </label>
            <label className={formChoiceLabelClass()}>
              <input
                type="radio"
                name={name}
                value="no"
                className="text-indigo-600 focus-visible:ring-2 focus-visible:ring-amber-500"
                checked={value === false}
                onChange={() => onChange(false)}
              />
              {t('eventsConsent.no')}
            </label>
          </div>
          <p id={hintId} className="mt-2 text-xs text-gray-600 dark:text-gray-400">
            {t('eventsConsent.hint')}
          </p>
          {status ? (
            <p id={statusId} className="mt-1.5 text-sm text-gray-800 dark:text-gray-200">
              {status}
            </p>
          ) : null}
        </fieldset>
      )}
    </div>
  );
}
