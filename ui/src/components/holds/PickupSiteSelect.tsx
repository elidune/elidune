import { useTranslation } from 'react-i18next';
import type { Site } from '@/types';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { pickupSiteRequired } from '@/utils/transitDisplay';

export interface PickupSiteSelectProps {
  sites: Site[];
  value: string;
  onChange: (id: string) => void;
  disabled?: boolean;
  id?: string;
  showHint?: boolean;
}

export default function PickupSiteSelect({
  sites,
  value,
  onChange,
  disabled = false,
  id = 'pickup-site',
  showHint = true,
}: PickupSiteSelectProps) {
  const { t } = useTranslation();
  if (sites.length === 0) return null;

  const required = pickupSiteRequired(sites);

  return (
    <div className="space-y-1">
      <label className={formLabelClass({ marginBottom: false })} htmlFor={id}>
        {t('holds.pickupSite')}
        {required ? <span className="text-red-600 dark:text-red-400"> *</span> : null}
      </label>
      <select
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        required={required}
        className={formControlClass({ className: 'w-full' })}
      >
        {!required && <option value="">{t('holds.pickupSiteAny')}</option>}
        {required && !value && <option value="">{t('holds.pickupSiteRequired')}</option>}
        {sites.map((site) => (
          <option key={site.id} value={site.id}>
            {site.name?.trim() || site.id}
          </option>
        ))}
      </select>
      {showHint && (
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('holds.pickupSiteHint')}</p>
      )}
    </div>
  );
}
