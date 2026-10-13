import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  audienceUnpaidThresholdForCreate,
  audienceUnpaidThresholdForUpdate,
  globalUnpaidThresholdBody,
  normalizeFinePolicy,
} from './finePolicy.ts';

const LOCALES = { fr, en, de, es } as const;

function wire(body: unknown): string {
  return JSON.stringify(body);
}

describe('unpaid threshold payloads', () => {
  it('rejects an amount mode without cents the same way the API returns 422', () => {
    assert.deepEqual(globalUnpaidThresholdBody('amount', ''), { ok: false });
    assert.deepEqual(globalUnpaidThresholdBody('amount', '  '), { ok: false });
    assert.deepEqual(globalUnpaidThresholdBody('amount', '-1'), { ok: false });
    assert.deepEqual(globalUnpaidThresholdBody('amount', 'abc'), { ok: false });
    assert.deepEqual(audienceUnpaidThresholdForCreate('amount', ''), { ok: false });
    assert.deepEqual(
      audienceUnpaidThresholdForUpdate({ mode: 'none', cents: null }, 'amount', ''),
      { ok: false },
    );
  });

  it('sends mode none without cents or the legacy decimal field', () => {
    const global = globalUnpaidThresholdBody('none', '12.50');
    assert.equal(global.ok, true);
    if (!global.ok) return;
    const json = wire(global.body);
    assert.deepEqual(global.body, { unpaidThresholdMode: 'none' });
    assert.equal(json.includes('unpaidThresholdCents'), false);
    assert.equal(json.includes('unpaidFineThreshold'), false);
    assert.equal(json.includes('null'), false);
  });

  it('converts euros to integer cents and always sends mode amount with them', () => {
    const global = globalUnpaidThresholdBody('amount', '10,50');
    assert.equal(global.ok, true);
    if (!global.ok) return;
    assert.deepEqual(global.body, {
      unpaidThresholdMode: 'amount',
      unpaidThresholdCents: 1050,
    });

    const created = audienceUnpaidThresholdForCreate('amount', '0');
    assert.equal(created.ok, true);
    if (!created.ok || !created.body) return;
    assert.deepEqual(created.body, {
      unpaidThresholdMode: 'amount',
      unpaidThresholdCents: 0,
    });
  });

  it('omits the mode on update when it is unchanged, but still sends amount when only cents change', () => {
    const untouched = audienceUnpaidThresholdForUpdate(
      { mode: 'inherit', cents: null },
      'inherit',
      '',
    );
    assert.deepEqual(untouched, { ok: true });

    const sameAmount = audienceUnpaidThresholdForUpdate(
      { mode: 'amount', cents: 450 },
      'amount',
      '4.50',
    );
    assert.deepEqual(sameAmount, { ok: true });

    const centsOnly = audienceUnpaidThresholdForUpdate(
      { mode: 'amount', cents: 450 },
      'amount',
      '5',
    );
    assert.equal(centsOnly.ok, true);
    if (!centsOnly.ok) return;
    assert.deepEqual(centsOnly.body, {
      unpaidThresholdMode: 'amount',
      unpaidThresholdCents: 500,
    });

    const cleared = audienceUnpaidThresholdForUpdate(
      { mode: 'amount', cents: 450 },
      'none',
      '4.50',
    );
    assert.equal(cleared.ok, true);
    if (!cleared.ok || !cleared.body) return;
    assert.deepEqual(cleared.body, { unpaidThresholdMode: 'none' });
    assert.equal(wire(cleared.body).includes('unpaidThresholdCents'), false);
    assert.equal(wire(cleared.body).includes('null'), false);
  });

  it('does not turn a legacy decimal threshold into a PUT body', () => {
    const policy = normalizeFinePolicy({ unpaidFineThreshold: '10.00' });
    assert.deepEqual(policy, { unpaidThresholdMode: 'none' });
    const body = globalUnpaidThresholdBody(policy.unpaidThresholdMode, '');
    assert.equal(body.ok, true);
    if (!body.ok) return;
    assert.equal(wire(body.body).includes('unpaidFineThreshold'), false);
  });
});

describe('unpaid threshold copy', () => {
  it('keeps the mode labels in every locale', () => {
    for (const messages of Object.values(LOCALES)) {
      const fine = messages.settings.finePolicy;
      const audience = messages.settings.publicTypes;
      assert.equal(typeof fine.modeNone, 'string');
      assert.equal(typeof fine.modeAmount, 'string');
      assert.equal(typeof fine.invalidAmount, 'string');
      assert.equal(typeof audience.unpaidThresholdModeInherit, 'string');
      assert.equal(typeof audience.unpaidThresholdModeNone, 'string');
      assert.equal(typeof audience.unpaidThresholdModeAmount, 'string');
      assert.equal(typeof audience.unpaidFineThresholdInherit, 'string');
      assert.equal(typeof audience.unpaidFineThresholdInheritEmpty, 'string');
    }
  });

  it('uses lecteur and exemplaire in French', () => {
    const text = `${fr.settings.finePolicy.help} ${fr.settings.publicTypes.unpaidFineThresholdHelp}`;
    assert.match(text, /lecteur/);
    assert.match(text, /exemplaire/);
    assert.doesNotMatch(text, /usager|spécimen|specimen|abonné/i);
  });
});
