import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { FineRule } from '@/types';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { withAccrueFines } from './reminderPolicy.ts';
import {
  audienceUnpaidThresholdForCreate,
  audienceUnpaidThresholdForUpdate,
  fineSettingsDraft,
  fineSettingsSavePlan,
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

describe('fine settings save plan', () => {
  const rules: FineRule[] = [
    { id: 1, mediaType: null, dailyRate: '0.20', maxAmount: '15.00', graceDays: 3 },
    { id: 2, mediaType: 'videoDvd', dailyRate: '1.00', graceDays: 1 },
  ];

  it('turns late fines off and leaves the stored threshold in place', () => {
    const plan = fineSettingsSavePlan({
      enabled: false,
      canControlAccrual: true,
      dailyRate: '0.20',
      blockMode: 'amount',
      threshold: '10.00',
      rules,
    });
    assert.equal(plan.ok, true);
    if (!plan.ok) return;
    assert.equal(plan.policy, null);
    assert.equal(plan.accrueFines, false);
    assert.equal(plan.rule, null);
  });

  it('keeps the stored grace and cap when the daily rate changes', () => {
    const plan = fineSettingsSavePlan({
      enabled: true,
      canControlAccrual: true,
      dailyRate: '0,50',
      blockMode: 'amount',
      threshold: '10',
      rules,
    });
    assert.equal(plan.ok, true);
    if (!plan.ok) return;
    assert.deepEqual(plan.policy, { unpaidThresholdMode: 'amount', unpaidThresholdCents: 1000 });
    assert.equal(plan.accrueFines, true);
    assert.deepEqual(plan.rule, {
      mediaType: null,
      dailyRate: '0.50',
      maxAmount: '15.00',
      graceDays: 3,
    });
  });

  it('does not write admin accrual when this session cannot', () => {
    const plan = fineSettingsSavePlan({
      enabled: true,
      canControlAccrual: false,
      dailyRate: '0.20',
      blockMode: 'none',
      threshold: '',
      rules,
    });
    assert.equal(plan.ok, true);
    if (!plan.ok) return;
    assert.equal(plan.accrueFines, null);
    assert.deepEqual(plan.policy, { unpaidThresholdMode: 'none' });
    assert.equal(plan.rule?.dailyRate, '0.20');
  });

  it('rejects an enabled policy with an empty rate or threshold', () => {
    assert.deepEqual(
      fineSettingsSavePlan({
        enabled: true,
        canControlAccrual: true,
        dailyRate: '',
        blockMode: 'none',
        threshold: '',
        rules,
      }),
      { ok: false, field: 'dailyRate' },
    );
    assert.deepEqual(
      fineSettingsSavePlan({
        enabled: true,
        canControlAccrual: true,
        dailyRate: '0.20',
        blockMode: 'amount',
        threshold: '',
        rules,
      }),
      { ok: false, field: 'threshold' },
    );
  });

  it('reads the default rule and the accrual flag into the draft', () => {
    const draft = fineSettingsDraft(
      { unpaidThresholdMode: 'amount', unpaidThresholdCents: 500 },
      false,
      rules,
    );
    assert.equal(draft.enabled, false);
    assert.equal(draft.dailyRate, '0.20');
    assert.equal(draft.blockMode, 'amount');
    assert.equal(draft.threshold, '5.00');
    assert.equal(draft.graceDays, 3);
    assert.equal(draft.maxAmount, '15.00');
    assert.equal(draft.hasMediaRules, true);
  });

  it('refuses to patch accrual onto an incomplete reminders payload', () => {
    assert.equal(withAccrueFines({}, false), null);
    const payload = withAccrueFines({ send_time: '09:00', accrueFines: true, overridable: true }, false);
    assert.deepEqual(payload, { send_time: '09:00', accrue_fines: false });
  });
});

describe('unpaid threshold copy', () => {
  it('keeps the mode labels in every locale', () => {
    const fineKeys = Object.keys(fr.settings.finePolicy).sort();
    for (const messages of Object.values(LOCALES)) {
      const fine = messages.settings.finePolicy;
      const audience = messages.settings.publicTypes;
      assert.deepEqual(Object.keys(fine).sort(), fineKeys);
      assert.equal(typeof fine.enabled, 'string');
      assert.equal(typeof fine.dailyRate, 'string');
      assert.equal(typeof fine.modeNone, 'string');
      assert.equal(typeof fine.modeAmount, 'string');
      assert.equal(typeof fine.invalidAmount, 'string');
      assert.equal(typeof audience.unpaidThresholdModeInherit, 'string');
      assert.equal(typeof audience.unpaidThresholdModeNone, 'string');
      assert.equal(typeof audience.unpaidThresholdModeAmount, 'string');
      assert.equal(typeof audience.unpaidFineThresholdInherit, 'string');
      assert.equal(typeof audience.unpaidFineThresholdInheritEmpty, 'string');
      assert.equal(typeof audience.finesDisabledNoEffect, 'string');
      assert.equal('unpaidFineThresholdInheritFinesOff' in audience, false);
      assert.equal('blockingStillActive' in fine, false);
    }
  });

  it('uses lecteur and exemplaire in French and does not keep a threshold active while fines are off', () => {
    const text = `${fr.settings.finePolicy.help} ${fr.settings.publicTypes.unpaidFineThresholdHelp} ${fr.settings.publicTypes.finesDisabledNoEffect}`;
    assert.match(text, /lecteur/);
    assert.match(text, /exemplaire/);
    assert.match(fr.settings.finePolicy.help, /perdu/);
    assert.match(fr.settings.finePolicy.help, /retard/);
    assert.match(fr.settings.finePolicy.help, /rappels/);
    assert.match(fr.settings.finePolicy.help, /ne bloquent plus/);
    assert.equal(fr.settings.publicTypes.finesDisabledNoEffect, 'Sans effet : amendes désactivées');
    assert.equal(en.settings.publicTypes.finesDisabledNoEffect, 'No effect: fines are disabled');
    assert.equal(en.settings.finePolicy.help.includes('still sent'), true);
    assert.equal(en.settings.finePolicy.help.includes('no longer block'), true);
    assert.doesNotMatch(text, /usager|spécimen|specimen|abonné/i);
    assert.doesNotMatch(text, /bloque encore|still blocks|sigue bloqueando|sperrt diesen Typ weiterhin/i);
  });
});
