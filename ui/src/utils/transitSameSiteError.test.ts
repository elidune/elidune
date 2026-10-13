import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import en from '../locales/en/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { getApiErrorCode, getApiErrorMessage } from './apiError.ts';

const FR =
  "L'exemplaire est déjà sur le site de retrait, aucun transit n'est nécessaire.";
const EN = 'The copy is already at the pickup site, no transit is needed.';

function sameSiteError() {
  return {
    isAxiosError: true,
    message: 'Request failed with status code 422',
    response: {
      status: 422,
      data: {
        code: 'transit_same_site',
        message: 'Item is already at the pickup site',
      },
    },
  };
}

async function tFor(lng: 'fr' | 'en'): Promise<TFunction> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    fallbackLng: 'en',
    resources: {
      fr: { translation: fr },
      en: { translation: en },
    },
    interpolation: { escapeValue: false },
  });
  return i18n.t.bind(i18n);
}

describe('start transit same-site error', () => {
  it('maps 422 transit_same_site to the French and English sentences', async () => {
    const err = sameSiteError();
    assert.equal(getApiErrorCode(err), 'transit_same_site');
    assert.equal(getApiErrorMessage(err, await tFor('fr')), FR);
    assert.equal(getApiErrorMessage(err, await tFor('en')), EN);
    assert.equal(getApiErrorMessage(err, await tFor('fr')).includes('already at the pickup'), false);
  });
});
