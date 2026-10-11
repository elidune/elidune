import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  applyCountryChange,
  countryCodeFromStored,
  formatUtcOffset,
  timezoneBelongsToCountry,
  timezoneChoiceRequired,
  timezoneMismatchesCountry,
  zonesForCountry,
} from './countryTimezone.ts';

describe('country time zones', () => {
  it('uses the only zone of a single-zone country', () => {
    const zones = zonesForCountry('FR');
    assert.equal(zones.length, 1);
    assert.equal(zones[0]?.id, 'Europe/Paris');
    assert.equal(zonesForCountry('RE')[0]?.id, 'Indian/Reunion');
    assert.equal(zonesForCountry('GP')[0]?.id, 'America/Guadeloupe');
    assert.equal(zonesForCountry('MQ')[0]?.id, 'America/Martinique');
    assert.equal(timezoneBelongsToCountry('Europe/Paris', 'FR'), true);
    assert.equal(timezoneMismatchesCountry('America/New_York', 'FR'), true);
    assert.equal(timezoneChoiceRequired('FR', ''), false);
  });

  it('does not pick a default zone for a multi-zone country', () => {
    const zones = zonesForCountry('US');
    assert.ok(zones.length > 1);
    assert.equal(zonesForCountry('DE').length, 2);
    assert.equal(timezoneChoiceRequired('US', ''), true);
    assert.equal(timezoneChoiceRequired('US', 'America/Chicago'), false);
    assert.equal(timezoneBelongsToCountry('America/Chicago', 'US'), true);
    assert.equal(timezoneBelongsToCountry('Europe/Paris', 'US'), false);
    assert.equal(zones.some((zone) => zone.fr === 'Est — New York'), true);
    assert.equal(zones.some((zone) => zone.fr === 'Centre — Chicago'), true);
  });

  it('sets the only zone on country change and clears a multi-zone country', () => {
    const mismatched = { country: 'FR', timezone: 'America/New_York' };
    assert.deepEqual(applyCountryChange(mismatched, 'FR'), mismatched);
    assert.deepEqual(applyCountryChange(mismatched, 'US'), {
      country: 'US',
      timezone: '',
    });
    assert.deepEqual(applyCountryChange({ country: 'US', timezone: 'America/Chicago' }, 'FR'), {
      country: 'FR',
      timezone: 'Europe/Paris',
    });
    assert.deepEqual(applyCountryChange({ country: 'US', timezone: '' }, 'RE'), {
      country: 'RE',
      timezone: 'Indian/Reunion',
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
