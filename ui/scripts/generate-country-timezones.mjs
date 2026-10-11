/**
 * Build `server/data/countryTimezones.json` from IANA tzdb `zone.tab`.
 *
 * chrono-tz exposes IANA names only; it has no country → zone table.
 * This JSON is the single country/zone list for the UI and the server
 * (`include_str!` from `server/src/services/country_zones.rs`).
 *
 * Source (public domain), usually under `/usr/share/zoneinfo`:
 * - `zone.tab` — one ISO country code per row, then coordinates, the zone,
 *   and an optional comment. Unlike `zone1970.tab`, a shared zone is not
 *   collapsed onto several countries, so Réunion, Guadeloupe, and Martinique
 *   keep `Indian/Reunion`, `America/Guadeloupe`, and `America/Martinique`.
 *   Override with `ZONE_TAB`.
 * - `iso3166.tab` — the alpha-2 codes tzdata ships. A `zone.tab` row is kept
 *   only when its code is in this list. Country labels in the UI come from
 *   `Intl.DisplayNames`, not from the English names in this file.
 *   Override with `ISO3166_TAB`.
 *
 * Rows stay in file order. There is no default zone: a country with one row
 * has that zone, and a country with several rows requires an explicit choice.
 *
 * Run from `ui/`: `node scripts/generate-country-timezones.mjs`
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const ZONE_TAB = process.env.ZONE_TAB || '/usr/share/zoneinfo/zone.tab';
const ISO3166_TAB = process.env.ISO3166_TAB || '/usr/share/zoneinfo/iso3166.tab';

const REGIONS = [
  [/alaska/i, 'alaska'],
  [/hawaii|honolulu/i, 'hawaii'],
  [/arizona|\bAZ\b/, 'arizona'],
  [/newfoundland/i, 'newfoundland'],
  [/eastern/i, 'eastern'],
  [/central/i, 'central'],
  [/mountain/i, 'mountain'],
  [/pacific/i, 'pacific'],
  [/atlantic/i, 'atlantic'],
];

const REGION_LABELS = {
  alaska: { en: 'Alaska', fr: 'Alaska', de: 'Alaska', es: 'Alaska' },
  hawaii: { en: 'Hawaii', fr: 'Hawaii', de: 'Hawaii', es: 'Hawái' },
  arizona: { en: 'Arizona', fr: 'Arizona', de: 'Arizona', es: 'Arizona' },
  newfoundland: { en: 'Newfoundland', fr: 'Terre-Neuve', de: 'Neufundland', es: 'Terranova' },
  eastern: { en: 'Eastern', fr: 'Est', de: 'Ost', es: 'Este' },
  central: { en: 'Central', fr: 'Centre', de: 'Mitte', es: 'Centro' },
  mountain: { en: 'Mountain', fr: 'Rocheuses', de: 'Rocky Mountains', es: 'Montañas' },
  pacific: { en: 'Pacific', fr: 'Pacifique', de: 'Pazifik', es: 'Pacífico' },
  atlantic: { en: 'Atlantic', fr: 'Atlantique', de: 'Atlantik', es: 'Atlántico' },
};

function cityName(zoneId) {
  return zoneId.split('/').at(-1).replaceAll('_', ' ');
}

function labelsFor(zoneId, comment) {
  const city = cityName(zoneId);
  const regionId = REGIONS.find(([pattern]) => pattern.test(comment))?.[1];
  if (!regionId) {
    return { en: city, fr: city, de: city, es: city };
  }
  const region = REGION_LABELS[regionId];
  return {
    en: `${region.en} — ${city}`,
    fr: `${region.fr} — ${city}`,
    de: `${region.de} — ${city}`,
    es: `${region.es} — ${city}`,
  };
}

function isoCodes(text) {
  const codes = new Set();
  for (const line of text.split('\n')) {
    if (!line.trim() || line.startsWith('#')) continue;
    const code = line.split('\t')[0];
    if (/^[A-Z]{2}$/.test(code)) codes.add(code);
  }
  return codes;
}

function parseZones(text, allowed) {
  /** @type {Map<string, { id: string, comment: string }[]>} */
  const byCountry = new Map();
  for (const line of text.split('\n')) {
    if (!line.trim() || line.startsWith('#')) continue;
    const parts = line.split('\t');
    if (parts.length < 3) continue;
    const code = parts[0];
    if (!/^[A-Z]{2}$/.test(code) || code.includes(',')) {
      throw new Error(`zone.tab row is not a single country code: ${line}`);
    }
    if (!allowed.has(code)) {
      console.warn(`skip ${code}: not in iso3166.tab`);
      continue;
    }
    const list = byCountry.get(code) ?? [];
    const id = parts[2];
    if (!list.some((zone) => zone.id === id)) {
      list.push({ id, comment: parts[3] ?? '' });
    }
    byCountry.set(code, list);
  }
  return byCountry;
}

const allowed = isoCodes(readFileSync(ISO3166_TAB, 'utf8'));
const byCountry = parseZones(readFileSync(ZONE_TAB, 'utf8'), allowed);
const countries = [...byCountry.keys()].sort();

const entries = countries.map((code) => {
  const zones = byCountry.get(code);
  return [code, zones.map((zone) => ({ id: zone.id, ...labelsFor(zone.id, zone.comment) }))];
});

const output = `${JSON.stringify(Object.fromEntries(entries), null, 2)}\n`;
const target = new URL('../../server/data/countryTimezones.json', import.meta.url);
mkdirSync(new URL('.', target), { recursive: true });
writeFileSync(target, output);
console.log(`wrote ${countries.length} countries from ${ZONE_TAB}`);
