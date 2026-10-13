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
  notice: {
    level: 'informational' | 'warning';
    code: 'item_not_on_loan' | 'item_not_available_status';
    circulationStatus: string | null;
    weedingStatus: string | null;
  },
  options?: { putBack?: boolean; onFound?: () => void },
): string {
  const tree = createElement(
    I18nextProvider,
    { i18n },
    createElement(ReturnDeskNotice, {
      notice,
      putBack: options?.putBack ?? false,
      onFound: options?.onFound,
    }),
  ) as ReactElement;
  return renderToStaticMarkup(tree);
}

describe('return desk notice banner', () => {
  it('shows an informational shelf note without success styling', async () => {
    const html = render(
      await i18nFor('fr'),
      { level: 'informational', code: 'item_not_on_loan', circulationStatus: null, weedingStatus: null },
      { putBack: true },
    );
    assert.match(html, /data-return-notice="informational"/);
    assert.match(html, /role="status"/);
    assert.match(html, /Cet exemplaire n&#x27;est pas en prêt à un lecteur/);
    assert.match(html, /Remettez-le en rayon/);
    assert.equal(html.includes('emerald'), false);
    assert.equal(html.includes(fr.loans.returnSuccess), false);
  });

  it('shows a lost copy as a warning, not a successful return', async () => {
    const html = render(
      await i18nFor('en'),
      {
        level: 'warning',
        code: 'item_not_available_status',
        circulationStatus: 'lost',
        weedingStatus: 'onShelf',
      },
      { onFound: () => undefined },
    );
    assert.match(html, /data-return-notice="warning"/);
    assert.match(html, /role="alert"/);
    assert.match(html, /Copy declared lost: use “Found” to put it back into circulation\./);
    assert.match(html, /data-return-action="lost-found"/);
    assert.match(html, />Found</);
    assert.match(html, /amber/);
    assert.equal(html.includes('emerald'), false);
    assert.equal(html.includes('Return successful'), false);
  });

  it('offers Retrouvé only for the server lost status when a barcode action exists', async () => {
    const frI18n = await i18nFor('fr');
    const lost = {
      level: 'warning' as const,
      code: 'item_not_available_status' as const,
      circulationStatus: 'lost',
      weedingStatus: 'onShelf',
    };
    const withAction = render(frI18n, lost, { onFound: () => undefined });
    assert.match(withAction, /Exemplaire déclaré perdu : utilisez « Retrouvé » pour le remettre en circulation\./);
    assert.match(withAction, /data-return-action="lost-found"/);
    assert.match(withAction, />Retrouvé</);

    const noBarcode = render(frI18n, lost);
    assert.match(noBarcode, /Exemplaire déclaré perdu/);
    assert.equal(noBarcode.includes('data-return-action'), false);
    assert.equal(noBarcode.includes('>Retrouvé<'), false);

    const damaged = render(
      frI18n,
      { ...lost, circulationStatus: 'damaged' },
      { onFound: () => undefined },
    );
    assert.match(damaged, /Statut : Abîmé/);
    assert.equal(damaged.includes('data-return-action'), false);
    assert.equal(damaged.includes('Retrouvé'), false);
  });
});
