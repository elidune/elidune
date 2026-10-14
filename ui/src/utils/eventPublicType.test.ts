import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { PublicType } from '../types/index.ts';
import { getApiErrorMessage } from './apiError.ts';
import {
  applyAllAudiencesToggle,
  eventAudienceDisplayLabel,
  eventAudienceErrorKey,
  eventSaveErrorMessage,
  resolveEventAudiences,
} from './eventPublicType.ts';

const catalogs = { en, fr, de, es };

function patronType(name: string, label: string): PublicType {
  return { id: name, name, label, unpaidThresholdMode: 'inherit' };
}

function httpError(status: number, code: string, message: string) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: { code, error: 'Validation Error', message } },
  };
}

const t = ((key: string) => {
  if (key === 'errors.apiCode.validation_error') return 'The submitted data is invalid.';
  if (key === 'codes.patronType.child') return 'Child';
  if (key === 'codes.patronType.adult') return 'Adult';
  return key;
}) as TFunction;

describe('event audiences', () => {
  it('rejects an empty selection and a combined selection', () => {
    assert.deepEqual(resolveEventAudiences({ allAudiences: false, publicTypes: [] }), {
      ok: false,
      reason: 'empty',
    });
    assert.deepEqual(resolveEventAudiences({ allAudiences: false, publicTypes: ['  '] }), {
      ok: false,
      reason: 'empty',
    });
    assert.deepEqual(resolveEventAudiences({ allAudiences: true, publicTypes: ['adult'] }), {
      ok: false,
      reason: 'combined',
    });
    assert.equal(eventAudienceErrorKey('empty'), 'events.targetPublicRequired');
    assert.equal(eventAudienceErrorKey('combined'), 'events.targetPublicCombined');
  });

  it('writes allAudiences or publicTypes, never legacy publicType', () => {
    const all = resolveEventAudiences({ allAudiences: true, publicTypes: ['  '] });
    assert.equal(all.ok, true);
    if (!all.ok) return;
    assert.deepEqual(all.payload, { allAudiences: true, publicTypes: [] });
    assert.equal('publicType' in all.payload, false);

    const named = resolveEventAudiences({
      allAudiences: false,
      publicTypes: [' adult ', 'adult', 'child'],
    });
    assert.equal(named.ok, true);
    if (!named.ok) return;
    assert.deepEqual(named.payload, { allAudiences: false, publicTypes: ['adult', 'child'] });
    assert.deepEqual(Object.keys(named.payload).sort(), ['allAudiences', 'publicTypes']);
  });

  it('clears the list when All audiences is turned on', () => {
    assert.deepEqual(
      applyAllAudiencesToggle({ allAudiences: false, publicTypes: ['child', 'adult'] }, true),
      { allAudiences: true, publicTypes: [] },
    );
    assert.deepEqual(
      applyAllAudiencesToggle({ allAudiences: true, publicTypes: [] }, false),
      { allAudiences: false, publicTypes: [] },
    );
  });

  it('shows the 400 server message instead of the generic validation translation', () => {
    const combined = httpError(400, 'validation_error', 'allAudiences cannot be combined with publicTypes');
    const empty = httpError(400, 'validation_error', 'Select at least one audience, or set allAudiences to true');
    assert.equal(getApiErrorMessage(combined, t), 'The submitted data is invalid.');
    assert.equal(eventSaveErrorMessage(combined, t), 'allAudiences cannot be combined with publicTypes');
    assert.equal(eventSaveErrorMessage(empty, t), 'Select at least one audience, or set allAudiences to true');
    assert.equal(
      eventSaveErrorMessage(httpError(500, 'internal_error', 'boom'), t),
      getApiErrorMessage(httpError(500, 'internal_error', 'boom'), t),
    );
  });

  it('lists every selected audience, or the all-audiences caption', () => {
    const types = [patronType('child', 'Child'), patronType('adult', 'Adult')];
    assert.equal(
      eventAudienceDisplayLabel({ allAudiences: true, publicTypes: [] }, types, 'All audiences', t),
      'All audiences',
    );
    assert.equal(
      eventAudienceDisplayLabel({ allAudiences: false, publicTypes: ['child', 'adult'] }, types, 'All audiences', t),
      'Child, Adult',
    );
    assert.equal(
      eventAudienceDisplayLabel({ allAudiences: false, publicTypes: [] }, types, 'All audiences', t),
      '—',
    );
  });

  it('has audience strings in French, English, German, and Spanish', () => {
    for (const catalog of Object.values(catalogs)) {
      assert.ok(catalog.events.targetPublic.all.trim());
      assert.ok(catalog.events.targetPublicHint.trim());
      assert.ok(catalog.events.targetPublicRequired.trim());
      assert.ok(catalog.events.targetPublicCombined.trim());
    }
    assert.notEqual(en.events.targetPublicCombined, fr.events.targetPublicCombined);
    assert.notEqual(en.events.targetPublicCombined, de.events.targetPublicCombined);
    assert.notEqual(en.events.targetPublicCombined, es.events.targetPublicCombined);
  });
});
