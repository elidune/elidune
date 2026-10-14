import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import type { ItemTransit } from '@/types';
import { normalizePaginatedResponse } from '@/utils/serverJson';
import { transitListPatron } from '@/utils/transitDisplay';
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
  it('renders the card with initials beside it and never a name or a reader link', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(TransitReaderCell, { patron: { cardNumber: 'CARD-3', initials: 'H.H.' } }),
    );
    assert.match(html, /carte n° CARD-3 H\.H\./);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('<a'), false);
    assert.equal(html.includes('spécimen'), false);
    assert.equal(html.includes('patron'), false);
  });

  it('renders spaced initials beside the card and never a name', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(TransitReaderCell, { patron: { cardNumber: 'CARD-42', initials: 'J. D.' } }),
    );
    assert.match(html, /carte n° CARD-42 J\. D\./);
    assert.equal(html.includes('Jean'), false);
    assert.equal(html.includes('Dupont'), false);
  });

  it('renders server initials exactly, including a value with no dots', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(TransitReaderCell, { patron: { cardNumber: 'CARD-42', initials: 'JD' } }),
    );
    assert.match(html, /carte n° CARD-42 JD/);
  });

  it('renders GET /transits items the way the transits page paints the reader column', async () => {
    // Default `/transits` filter loads two list responses and concatenates `items`.
    // `listTransits` returns `normalizePaginatedResponse` with each item unchanged.
    const listBody = (row: unknown) =>
      JSON.parse(`{
        "items": [${JSON.stringify(row)}],
        "total": 1,
        "page": 1,
        "perPage": 200,
        "pageCount": 1
      }`) as { items: unknown[] };
    const jdRow = {
      id: '9001',
      itemId: '8001',
      holdId: '7001',
      barcode: 'EX-1',
      title: 'Privacy',
      sourceId: '10',
      sourceName: 'Annex',
      patron: { cardNumber: 'CARD-42', initials: 'JD' },
      fromSourceId: '11',
      toSourceId: '10',
      status: 'inTransit',
      notes: null,
      createdAt: '2026-10-14T08:00:00Z',
      shippedAt: '2026-10-14T09:00:00Z',
      receivedAt: null,
      cancelledAt: null,
      shippedBy: '1',
      receivedBy: null,
      cancelledBy: null,
      reversedFromId: null,
    };
    const requested = normalizePaginatedResponse<ItemTransit>(
      listBody({ ...jdRow, id: '9000', status: 'requested', patron: { cardNumber: 'CARD-7', initials: 'JD' } }),
    );
    const inTransit = normalizePaginatedResponse<ItemTransit>(listBody(jdRow));
    const rows = [...requested.items, ...inTransit.items];
    const i18n = await i18nFor('fr');
    for (const row of rows) {
      const patron = transitListPatron(row);
      const desktop = render(i18n, createElement(TransitReaderCell, { patron }));
      const text = desktop.replace(/<[^>]+>/g, '');
      assert.equal(text, `carte n° ${row.patron?.cardNumber} JD`);
      assert.equal(text.endsWith('JD'), true);
      assert.equal(text.includes(' '), true);
      const mobile = patron
        ? render(i18n, createElement(TransitReaderCell, { patron }))
        : '';
      assert.equal(mobile.replace(/<[^>]+>/g, ''), text);
    }
  });

  it('renders initials when the reader has no card', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(TransitReaderCell, { patron: { cardNumber: null, initials: 'HH' } }),
    );
    assert.match(html, /HH/);
    assert.equal(html.includes('card number'), false);
  });

  it('renders nothing when the reader is null or initials are a name', async () => {
    const i18n = await i18nFor('fr');
    const empty = render(i18n, createElement(TransitReaderCell, { patron: null }));
    assert.equal(empty.includes('data-transit-reader'), false);
    assert.equal(empty.includes('—'), false);
    assert.equal(empty.includes('Helene'), false);

    const named = render(
      i18n,
      createElement(TransitReaderCell, { patron: { cardNumber: '  ', initials: 'Helene Holder' } }),
    );
    assert.equal(named.includes('data-transit-reader'), false);
    assert.equal(named.includes('—'), false);
    assert.equal(named.includes('Helene'), false);
    assert.equal(named.includes('Holder'), false);

    const cardWithoutName = render(
      i18n,
      createElement(TransitReaderCell, { patron: { cardNumber: 'CARD-3', initials: 'Helene Holder' } }),
    );
    assert.match(cardWithoutName, /carte n° CARD-3/);
    assert.equal(cardWithoutName.includes('Helene'), false);
    assert.equal(cardWithoutName.includes('Holder'), false);
    assert.equal(cardWithoutName.includes('HH'), false);
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
