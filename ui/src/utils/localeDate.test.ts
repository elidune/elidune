import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { formatLocaleDate, renewedUntilMessage } from './localeDate.ts';

const catalogs = { en, fr, de, es } as const;
type Lang = keyof typeof catalogs;

const DUE = '2026-10-13T12:00:00.000Z';

async function tFor(lng: Lang): Promise<TFunction> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
    interpolation: { escapeValue: false },
  });
  return i18n.t.bind(i18n) as TFunction;
}

describe('renewal confirmation', () => {
  it('toasts the new due date from POST /loans/{id}/renew expiryAt', () => {
    const page = readFileSync(new URL('../pages/MyLoansPage.tsx', import.meta.url), 'utf8');
    assert.match(page, /renewedUntilMessage\(renewed\.expiryAt,\s*t,\s*i18n\.language\)/);
    assert.match(page, /showToast\(\{\s*variant:\s*'success'/);
  });

  it('includes the localized due date in en, fr, de, and es', async () => {
    for (const lang of Object.keys(catalogs) as Lang[]) {
      const t = await tFor(lang);
      const date = formatLocaleDate(DUE, lang);
      const message = renewedUntilMessage(DUE, t, lang);
      assert.match(message, new RegExp(date.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
      assert.equal(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u.test(message), false);
      assert.notEqual(message, catalogs[lang].loans.renewSuccess);
    }
    const frT = await tFor('fr');
    assert.match(renewedUntilMessage(DUE, frT, 'fr'), /^Prolongé jusqu'au /);
    const enT = await tFor('en');
    assert.match(renewedUntilMessage(DUE, enT, 'en'), /^Renewed until /);
    const deT = await tFor('de');
    assert.match(renewedUntilMessage(DUE, deT, 'de'), /^Verlängert bis zum /);
    const esT = await tFor('es');
    assert.match(renewedUntilMessage(DUE, esT, 'es'), /^Renovado hasta el /);
  });

  it('falls back when the renew response has no due date', async () => {
    const t = await tFor('fr');
    assert.equal(renewedUntilMessage(undefined, t, 'fr'), 'Emprunt prolongé');
    assert.equal(renewedUntilMessage('   ', t, 'fr'), 'Emprunt prolongé');
  });
});
