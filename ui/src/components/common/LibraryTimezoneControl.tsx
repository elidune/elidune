import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import {
  formatUtcOffset,
  timezoneMismatchesCountry,
  zoneLabel,
  zonesForCountry,
  type CountryZone,
} from '@/utils/countryTimezone';

interface LibraryTimezoneControlProps {
  country: string;
  timezone: string;
  onTimezoneChange: (timezone: string) => void;
  error?: string;
}

function optionLabel(zone: CountryZone, language: string): string {
  return `${zoneLabel(zone, language)} (${formatUtcOffset(zone.id)})`;
}

/**
 * Time zone tied to the selected country.
 * One zone: a read-only derived hint. Several zones: a short select.
 * A saved zone from another country stays selected and shows a warning.
 */
export default function LibraryTimezoneControl({
  country,
  timezone,
  onTimezoneChange,
  error,
}: LibraryTimezoneControlProps) {
  const { t, i18n } = useTranslation();
  const language = i18n.resolvedLanguage || i18n.language || 'en';
  const zones = zonesForCountry(country);
  const mismatch = timezoneMismatchesCountry(timezone, country);

  const options = useMemo(() => {
    if (!mismatch || zones.some((zone) => zone.id === timezone)) return zones;
    const foreign: CountryZone = { id: timezone, en: timezone, fr: timezone, de: timezone, es: timezone };
    return [foreign, ...zones];
  }, [mismatch, timezone, zones]);

  if (!country || zones.length === 0) return null;

  if (zones.length === 1 && !mismatch) {
    const zone = zones[0].id;
    return (
      <p className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-950 dark:border-amber-800/80 dark:bg-amber-950/40 dark:text-amber-100">
        {t('library.general.timezoneDerived', {
          zone,
          offset: formatUtcOffset(zone),
        })}
      </p>
    );
  }

  return (
    <div>
      <label className={formLabelClass()}>{t('library.general.timezone')}</label>
      <select
        value={timezone}
        onChange={(event) => onTimezoneChange(event.target.value)}
        className={formControlClass({ error: !!error, className: 'w-full' })}
        aria-invalid={error || mismatch ? true : undefined}
      >
        {options.map((zone) => (
          <option key={zone.id} value={zone.id}>
            {optionLabel(zone, language)}
          </option>
        ))}
      </select>
      {mismatch && (
        <p className="mt-1 text-sm text-amber-700 dark:text-amber-300" role="status">
          {t('library.general.timezoneMismatch')}
        </p>
      )}
      {error && <p className="mt-1 text-sm text-red-600 dark:text-red-400">{error}</p>}
    </div>
  );
}
