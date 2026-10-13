import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import { TransitBarcodeCell, TransitReaderCell, TransitTitleCell } from './TransitQueueCells.tsx';

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
    },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nextProvider, { i18n }, node) as ReactElement);
}

describe('transit queue cells', () => {
  it('renders the card and never a name or a reader link', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(TransitReaderCell, { patron: { cardNumber: 'CARD-3', initials: 'HH' } }),
    );
    assert.match(html, /carte n°/);
    assert.match(html, /CARD-3/);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('<a'), false);
    assert.equal(html.includes('spécimen'), false);
  });

  it('renders initials when the reader has no card', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(TransitReaderCell, { patron: { cardNumber: null, initials: 'HH' } }),
    );
    assert.match(html, /HH/);
    assert.equal(html.includes('card number'), false);
  });

  it('renders an em dash when patron is null or a name slipped into initials', async () => {
    const i18n = await i18nFor('fr');
    const empty = render(i18n, createElement(TransitReaderCell, { patron: null }));
    assert.match(empty, /—/);
    assert.equal(empty.includes('Helene'), false);

    const named = render(
      i18n,
      createElement(TransitReaderCell, { patron: { cardNumber: '  ', initials: 'Helene Holder' } }),
    );
    assert.match(named, /—/);
    assert.equal(named.includes('Helene'), false);
  });

  it('renders the barcode and title from the transit, with a dash when they are absent', async () => {
    const i18n = await i18nFor('en');
    const filled = render(
      i18n,
      createElement(
        'div',
        null,
        createElement(TransitBarcodeCell, { barcode: 'EX-1' }),
        createElement(TransitTitleCell, { title: 'Privacy' }),
      ),
    );
    assert.match(filled, /EX-1/);
    assert.match(filled, /Privacy/);
    assert.equal(filled.includes('/biblios/'), false);

    const empty = render(
      i18n,
      createElement(
        'div',
        null,
        createElement(TransitBarcodeCell, { barcode: null }),
        createElement(TransitTitleCell, { title: null }),
      ),
    );
    assert.match(empty, /—/);
  });
});
