/** Library default when the API omits a time zone (migration 042). */
export const DEFAULT_LIBRARY_TIMEZONE = 'Europe/Paris';

/**
 * Curated IANA names used when `Intl.supportedValuesOf('timeZone')` is missing.
 * Every entry is a zone chrono-tz accepts.
 */
const FALLBACK_TIME_ZONES = [
  'Africa/Cairo',
  'Africa/Johannesburg',
  'Africa/Lagos',
  'America/Anchorage',
  'America/Argentina/Buenos_Aires',
  'America/Bogota',
  'America/Chicago',
  'America/Denver',
  'America/Los_Angeles',
  'America/Mexico_City',
  'America/New_York',
  'America/Sao_Paulo',
  'America/Toronto',
  'Asia/Dubai',
  'Asia/Hong_Kong',
  'Asia/Kolkata',
  'Asia/Shanghai',
  'Asia/Singapore',
  'Asia/Tokyo',
  'Australia/Sydney',
  'Europe/Berlin',
  'Europe/Brussels',
  'Europe/Lisbon',
  'Europe/London',
  'Europe/Madrid',
  'Europe/Paris',
  'Europe/Rome',
  'Europe/Zurich',
  'Pacific/Auckland',
  'Pacific/Honolulu',
];

function supportedTimeZones(): string[] | null {
  const intl = Intl as typeof Intl & {
    supportedValuesOf?: (key: string) => string[];
  };
  if (typeof intl.supportedValuesOf !== 'function') return null;
  try {
    const zones = intl.supportedValuesOf('timeZone');
    if (!Array.isArray(zones) || zones.length === 0) return null;
    return zones.filter((zone) => typeof zone === 'string' && zone.trim() !== '');
  } catch {
    return null;
  }
}

/** IANA time zones, from the runtime when available, otherwise {@link FALLBACK_TIME_ZONES}. */
export function ianaTimeZones(): string[] {
  const zones = supportedTimeZones() ?? FALLBACK_TIME_ZONES;
  return [...zones].sort((a, b) => a.localeCompare(b));
}

/** Options for the library time zone select, always including the current value. */
export function libraryTimeZoneOptions(current: string): string[] {
  const zones = ianaTimeZones();
  const trimmed = current.trim();
  if (trimmed && !zones.includes(trimmed)) {
    return [trimmed, ...zones];
  }
  return zones;
}
