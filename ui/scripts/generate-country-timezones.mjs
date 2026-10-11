/**
 * Build `src/data/countryTimezones.json` from IANA tzdb `zone1970.tab`.
 *
 * chrono-tz exposes IANA names only; it has no country → zone table.
 * This JSON is the single country/zone list for the UI and the server
 * (`include_str!` from `server/src/services/country_zones.rs`).
 *
 * Source (public domain): the tzdata file shipped with the OS, usually
 * `/usr/share/zoneinfo/zone1970.tab`. Override with `ZONE1970_TAB`.
 * The table lists each zone once, with ISO 3166-1 alpha-2 codes in column 1.
 * Column 4 comments exist only for countries that span more than one zone.
 *
 * The first entry for a country is its default zone: an explicit primary when
 * zone1970's geographical order is not the main population centre, otherwise
 * the first row in the file.
 *
 * Run from `ui/`: `node scripts/generate-country-timezones.mjs`
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';

const SOURCE = process.env.ZONE1970_TAB || '/usr/share/zoneinfo/zone1970.tab';

/** Main zone when zone1970 lists a fringe zone first. */
const PRIMARY = {
  AU: 'Australia/Sydney',
  BR: 'America/Sao_Paulo',
  CA: 'America/Toronto',
  CD: 'Africa/Lagos',
  DE: 'Europe/Berlin',
  MY: 'Asia/Singapore',
  RU: 'Europe/Moscow',
  UA: 'Europe/Kyiv',
  UZ: 'Asia/Tashkent',
  VN: 'Asia/Ho_Chi_Minh',
};

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

function parse(text) {
  /** @type {Map<string, { id: string, comment: string }[]>} */
  const byCountry = new Map();
  for (const line of text.split('\n')) {
    if (!line.trim() || line.startsWith('#')) continue;
    const parts = line.split('\t');
    if (parts.length < 3) continue;
    const comment = parts[3] ?? '';
    const id = parts[2];
    for (const code of parts[0].split(',')) {
      const list = byCountry.get(code) ?? [];
      if (!list.some((zone) => zone.id === id)) list.push({ id, comment });
      byCountry.set(code, list);
    }
  }
  return byCountry;
}

function pickDefault(code, zones) {
  const primary = PRIMARY[code];
  if (primary && zones.some((zone) => zone.id === primary)) return primary;
  return zones[0].id;
}

const byCountry = parse(readFileSync(SOURCE, 'utf8'));
const countries = [...byCountry.keys()].sort();

const entries = countries.map((code) => {
  const zones = byCountry.get(code);
  const defaultId = pickDefault(code, zones);
  const ordered = [
    zones.find((zone) => zone.id === defaultId),
    ...zones.filter((zone) => zone.id !== defaultId),
  ];
  return [code, ordered.map((zone) => ({ id: zone.id, ...labelsFor(zone.id, zone.comment) }))];
});

const output = `${JSON.stringify(Object.fromEntries(entries), null, 2)}\n`;

const target = new URL('../src/data/countryTimezones.json', import.meta.url);
mkdirSync(new URL('.', target), { recursive: true });
writeFileSync(target, output);
console.log(`wrote ${countries.length} countries from ${SOURCE}`);
