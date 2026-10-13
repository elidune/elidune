import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import ConfirmDialog from './ConfirmDialog.tsx';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

async function i18nFor(lng: Lang): Promise<I18nInstance> {
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
  return i18n;
}

function renderDialog(i18n: I18nInstance, title: string): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(ConfirmDialog, {
        isOpen: true,
        onClose: () => undefined,
        onConfirm: () => undefined,
        title,
        message: 'The copy stays on the hold shelf.',
      }),
    ) as ReactElement,
  );
}

describe('dialog title', () => {
  it('uses reader and copy wording for the hold warning', () => {
    assert.equal(fr.loans.toPickForOtherTitle, 'Exemplaire réservé pour un autre lecteur');
    assert.equal(fr.loans.toPickForOtherTitle.includes('usager'), false);
    assert.equal(en.loans.toPickForOtherTitle, 'Copy on hold for another reader');
    assert.equal(en.loans.toPickForOtherTitle.toLowerCase().includes('patron'), false);
    assert.equal(de.loans.toPickForOtherTitle, 'Exemplar für einen anderen Leser vorgemerkt');
    assert.equal(es.loans.toPickForOtherTitle, 'Ejemplar reservado para otro lector');
  });

  it('wraps the full title and exposes it as a tooltip', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const title = catalogs[lang].loans.toPickForOtherTitle;
      const html = renderDialog(await i18nFor(lang), title);
      const heading = new RegExp(
        `<h2[^>]*title="${title}"[^>]*>${title}</h2>`,
      ).exec(html);
      assert.ok(heading, html);
      assert.match(heading[0], /whitespace-normal/);
      assert.match(heading[0], /break-words/);
      assert.equal(heading[0].includes('truncate'), false);
      assert.equal(heading[0].includes('line-clamp'), false);
      assert.equal(heading[0].includes('text-balance'), false);
      assert.equal(heading[0].includes('whitespace-nowrap'), false);
    }
  });
});
