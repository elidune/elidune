import { COUNTRY_ZONES, type CountryZone } from '@/data/countryTimezones';

export type { CountryZone };

export function isKnownCountryCode(code: string): boolean {
  return Object.prototype.hasOwnProperty.call(COUNTRY_ZONES, code.toUpperCase());
}

/** ISO alpha-2 when `value` is a known code; otherwise empty (free text must be chosen again). */
export function countryCodeFromStored(value: string | null | undefined): string {
  const raw = value?.trim() ?? '';
  if (!/^[A-Za-z]{2}$/.test(raw)) return '';
  const code = raw.toUpperCase();
  return isKnownCountryCode(code) ? code : '';
}

export function zonesForCountry(code: string): readonly CountryZone[] {
  return COUNTRY_ZONES[code.toUpperCase()] ?? [];
}

/** A multi-zone country still needs an explicit zone. */
export function timezoneChoiceRequired(country: string, timezone: string): boolean {
  return zonesForCountry(country).length > 1 && !timezone.trim();
}

export function timezoneBelongsToCountry(timezone: string, code: string): boolean {
  const id = timezone.trim();
  if (!id || !isKnownCountryCode(code)) return false;
  return zonesForCountry(code).some((zone) => zone.id === id);
}

/** Saved zone is kept, but it is not one of the country's zones. */
export function timezoneMismatchesCountry(timezone: string, code: string): boolean {
  if (!code || !timezone.trim() || !isKnownCountryCode(code)) return false;
  return !timezoneBelongsToCountry(timezone, code);
}

/**
 * The same country leaves the current zone alone, including a mismatch.
 * A country with one zone selects that zone. A country with several zones
 * clears the choice so the user must pick one.
 */
export function applyCountryChange(
  current: { country: string; timezone: string },
  nextCountry: string,
): { country: string; timezone: string } {
  if (nextCountry === current.country) return current;
  const zones = zonesForCountry(nextCountry);
  return {
    country: nextCountry,
    timezone: zones.length === 1 ? zones[0].id : '',
  };
}

export function zoneLabel(zone: CountryZone, language: string): string {
  const lang = language.toLowerCase().split('-')[0];
  if (lang === 'fr') return zone.fr;
  if (lang === 'de') return zone.de;
  if (lang === 'es') return zone.es;
  return zone.en;
}

/** Offset for `date` in `timeZone`, such as `UTC+2` or `UTC-4`. DST follows `date`. */
export function formatUtcOffset(timeZone: string, date = new Date()): string {
  try {
    const parts = new Intl.DateTimeFormat('en-US', {
      timeZone,
      hour: '2-digit',
      timeZoneName: 'shortOffset',
    }).formatToParts(date);
    const raw = parts.find((part) => part.type === 'timeZoneName')?.value ?? 'GMT';
    const offset = raw.replace(/^GMT/, 'UTC');
    if (offset === 'UTC') return 'UTC+0';
    return offset;
  } catch {
    return 'UTC';
  }
}

/** Localized region name for an ISO code. Anything else is returned unchanged. */
export function formatCountryName(value: string | null | undefined, language: string): string {
  const raw = value?.trim() ?? '';
  if (!raw) return '';
  if (!/^[A-Za-z]{2}$/.test(raw)) return raw;
  const code = raw.toUpperCase();
  try {
    const name = new Intl.DisplayNames([language || 'en'], { type: 'region' }).of(code);
    return name && name.toUpperCase() !== code ? name : raw;
  } catch {
    return raw;
  }
}

export function countryCodes(): string[] {
  return Object.keys(COUNTRY_ZONES);
}
