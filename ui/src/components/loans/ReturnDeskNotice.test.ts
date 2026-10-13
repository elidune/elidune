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
import ReturnDeskNotice from './ReturnDeskNotice.tsx';

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
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

function render(
  i18n: I18nInstance,
  notice: { level: 'informational' | 'warning'; code: 'item_not_on_loan' | 'item_not_available_status'; statusLabelKey: string | null },
  putBack = false,
): string {
  const tree = createElement(
    I18nextProvider,
    { i18n },
    createElement(ReturnDeskNotice, { notice, putBack }),
  ) as ReactElement;
  return renderToStaticMarkup(tree);
}

describe('return desk notice banner', () => {
  it('shows an informational shelf note without success styling', async () => {
    const html = render(
      await i18nFor('fr'),
      { level: 'informational', code: 'item_not_on_loan', statusLabelKey: null },
      true,
    );
    assert.match(html, /data-return-notice="informational"/);
    assert.match(html, /role="status"/);
    assert.match(html, /Cet exemplaire n&#x27;est pas en prêt à un lecteur/);
    assert.match(html, /Remettez-le en rayon/);
    assert.equal(html.includes('emerald'), false);
    assert.equal(html.includes(fr.loans.returnSuccess), false);
  });

  it('shows a lost copy as a warning, not a successful return', async () => {
    const html = render(await i18nFor('en'), {
      level: 'warning',
      code: 'item_not_available_status',
      statusLabelKey: 'loans.exceptions.lost',
    });
    assert.match(html, /data-return-notice="warning"/);
    assert.match(html, /role="alert"/);
    assert.match(html, /This copy was not returned to a reader\. Status: Lost\./);
    assert.match(html, /amber/);
    assert.equal(html.includes('emerald'), false);
    assert.equal(html.includes('Return successful'), false);
  });
});
