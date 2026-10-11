import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  applyCountryChange,
  countryCodeFromStored,
  defaultTimezoneForCountry,
  formatUtcOffset,
  timezoneBelongsToCountry,
  timezoneMismatchesCountry,
  zonesForCountry,
} from './countryTimezone.ts';

describe('country time zones', () => {
  it('uses a single default zone for France', () => {
    const zones = zonesForCountry('FR');
    assert.equal(zones.length, 1);
    assert.equal(zones[0]?.id, 'Europe/Paris');
    assert.equal(defaultTimezoneForCountry('fr'), 'Europe/Paris');
    assert.equal(timezoneBelongsToCountry('Europe/Paris', 'FR'), true);
    assert.equal(timezoneMismatchesCountry('America/New_York', 'FR'), true);
  });

  it('defaults a multi-zone country to its main zone', () => {
    const zones = zonesForCountry('US');
    assert.ok(zones.length > 1);
    assert.equal(defaultTimezoneForCountry('US'), 'America/New_York');
    assert.equal(timezoneBelongsToCountry('America/Chicago', 'US'), true);
    assert.equal(timezoneBelongsToCountry('Europe/Paris', 'US'), false);
    assert.equal(zones.some((zone) => zone.fr === 'Est — New York'), true);
    assert.equal(zones.some((zone) => zone.fr === 'Centre — Chicago'), true);
  });

  it('resets the zone only when the country changes', () => {
    const mismatched = { country: 'FR', timezone: 'America/New_York' };
    assert.deepEqual(applyCountryChange(mismatched, 'FR'), mismatched);
    assert.deepEqual(applyCountryChange(mismatched, 'US'), {
      country: 'US',
      timezone: 'America/New_York',
    });
    assert.deepEqual(applyCountryChange({ country: 'US', timezone: 'America/Chicago' }, 'FR'), {
      country: 'FR',
      timezone: 'Europe/Paris',
    });
  });

  it('treats free-text countries as empty and keeps real codes', () => {
    assert.equal(countryCodeFromStored('France'), '');
    assert.equal(countryCodeFromStored('france'), '');
    assert.equal(countryCodeFromStored('FR'), 'FR');
    assert.equal(countryCodeFromStored('fr'), 'FR');
    assert.equal(countryCodeFromStored(''), '');
    assert.equal(countryCodeFromStored(null), '');
  });

  it('formats the offset for the given civil date, including DST', () => {
    assert.equal(formatUtcOffset('Europe/Paris', new Date('2026-07-16T12:00:00Z')), 'UTC+2');
    assert.equal(formatUtcOffset('Europe/Paris', new Date('2026-01-15T12:00:00Z')), 'UTC+1');
    assert.equal(formatUtcOffset('America/New_York', new Date('2026-07-16T12:00:00Z')), 'UTC-4');
  });
});
