import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';

import { serialFrequencyLabelKey } from './serialFrequencyLabel.ts';

describe('serial frequency labels', () => {
  it('maps the French catalog names without reading the English seed', () => {
    const french = JSON.parse(
      readFileSync(new URL('../locales/fr/translation.json', import.meta.url), 'utf8'),
    ) as { serials: { frequencies: Record<string, string> } };
    const labels = french.serials.frequencies;
    const named: Array<[string | null, number, number, string]> = [
      ['day', 1, 1, 'quotidien'],
      ['week', 1, 1, 'hebdomadaire'],
      ['week', 1, 2, 'bimensuel'],
      ['month', 1, 1, 'mensuel'],
      ['month', 1, 2, 'bimestriel'],
      ['month', 1, 3, 'trimestriel'],
      ['month', 1, 6, 'semestriel'],
      ['year', 1, 1, 'annuel'],
    ];
    for (const [unit, issues, units, label] of named) {
      const key = serialFrequencyLabelKey(unit, issues, units);
      assert.ok(key);
      const leaf = key.slice('serials.frequencies.'.length);
      assert.equal(labels[leaf], label);
    }
    assert.equal(serialFrequencyLabelKey('month', 4, 1), null);
    assert.equal(serialFrequencyLabelKey(null, 1, 1), 'serials.frequencies.irregular');
  });
});
