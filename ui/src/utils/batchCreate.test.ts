import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { batchCreateItemMessage } from './batchCreate.ts';

const catalogs = { fr, en, de, es } as const;

function lookup(catalog: unknown, key: string): string | null {
  const parts = key.split('.');
  let cursor: unknown = catalog;
  for (const part of parts) {
    if (!cursor || typeof cursor !== 'object' || !(part in cursor)) return null;
    cursor = (cursor as Record<string, unknown>)[part];
  }
  return typeof cursor === 'string' ? cursor : null;
}

function translate(lang: keyof typeof catalogs) {
  const catalog = catalogs[lang];
  return (key: string) => lookup(catalog, key) ?? key;
}

const OTHER = 'item_on_loan_to_other_borrower';
const SAME = 'item_already_on_loan_to_borrower';
const ENGLISH_OTHER = 'This item is on loan to another borrower.';

describe('batch-create item message', () => {
  it('translates a known code and ignores the English error string', () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const t = translate(lang);
      assert.equal(
        batchCreateItemMessage({ code: OTHER, error: ENGLISH_OTHER }, t),
        catalogs[lang].errors.apiCode.item_on_loan_to_other_borrower,
      );
      assert.equal(
        batchCreateItemMessage({ code: SAME, error: 'Renew the existing loan.' }, t),
        catalogs[lang].errors.apiCode.item_already_on_loan_to_borrower,
      );
    }
  });

  it('uses the generic error when the code is unknown, not the English string', () => {
    const item = { code: 'not_a_real_code', error: ENGLISH_OTHER };
    assert.equal(batchCreateItemMessage(item, translate('fr')), fr.errors.generic);
    assert.equal(batchCreateItemMessage(item, translate('en')), en.errors.generic);
    assert.equal(batchCreateItemMessage(item, translate('de')), de.errors.generic);
    assert.equal(batchCreateItemMessage(item, translate('es')), es.errors.generic);
    assert.equal(batchCreateItemMessage(item, translate('fr')).includes(ENGLISH_OTHER), false);
  });

  it('uses the server error string only when code is missing', () => {
    assert.equal(batchCreateItemMessage({ error: ENGLISH_OTHER }, translate('fr')), ENGLISH_OTHER);
    assert.equal(batchCreateItemMessage({ code: null, error: ENGLISH_OTHER }, translate('en')), ENGLISH_OTHER);
    assert.equal(batchCreateItemMessage({ code: '  ', error: ENGLISH_OTHER }, translate('de')), ENGLISH_OTHER);
    assert.equal(batchCreateItemMessage({ code: null, error: null }, translate('es')), es.errors.generic);
    assert.equal(batchCreateItemMessage({}, translate('fr')), fr.errors.generic);
  });
});
