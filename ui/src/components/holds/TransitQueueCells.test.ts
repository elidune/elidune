import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import type { HoldDetailsStaff } from '@/types';
import { transitListReader } from '@/utils/transitDisplay';
import { TransitBarcodeCell, TransitReaderCell, TransitTitleCell } from './TransitQueueCells.tsx';

const NAMED: HoldDetailsStaff['user'] = {
  id: 'u1',
  firstname: 'Helene',
  lastname: 'Holder',
  login: 'helene',
  barcode: 'CARD-3',
};

function hold(patch: Partial<HoldDetailsStaff> = {}): HoldDetailsStaff {
  return {
    id: 'h1',
    userId: 'u1',
    biblioId: 'b1',
    itemId: 'i1',
    pickupSiteId: 'site-main',
    biblio: {
      id: 'b1',
      mediaType: 'printedText',
      isbn: null,
      title: 'Privacy',
      date: null,
      status: 0,
      isValid: true,
      archivedAt: null,
      weedingStatus: 'onShelf',
      author: null,
      items: [],
    },
    user: NAMED,
    createdAt: '2026-10-01T00:00:00Z',
    notifiedAt: null,
    expiresAt: null,
    status: 'pending',
    position: 1,
    notes: null,
    ...patch,
  };
}

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
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(MemoryRouter, null, node)) as ReactElement,
  );
}

describe('transit queue cells', () => {
  it('renders the card and a reader link, never the name', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(TransitReaderCell, { reader: transitListReader(hold()), linked: true }),
    );
    assert.match(html, /carte n°/);
    assert.match(html, /CARD-3/);
    assert.match(html, /href="\/users\/u1"/);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('spécimen'), false);
  });

  it('renders initials when the reader has no card', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(TransitReaderCell, {
        reader: transitListReader(
          hold({ user: { id: 'u1', firstname: 'Helene', lastname: 'Holder', barcode: '  ' } }),
        ),
        linked: false,
      }),
    );
    assert.match(html, /HH/);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('<a'), false);
  });

  it('renders the barcode and title fields, with a dash when they are absent', async () => {
    const i18n = await i18nFor('en');
    const filled = render(
      i18n,
      createElement(
        'div',
        null,
        createElement(TransitBarcodeCell, { barcode: 'EX-1' }),
        createElement(TransitTitleCell, { title: 'Privacy', biblioId: 'b1' }),
      ),
    );
    assert.match(filled, /EX-1/);
    assert.match(filled, /Privacy/);
    assert.match(filled, /href="\/biblios\/b1"/);

    const empty = render(
      i18n,
      createElement(
        'div',
        null,
        createElement(TransitBarcodeCell, { barcode: null }),
        createElement(TransitTitleCell, { title: null, biblioId: 'b1' }),
      ),
    );
    assert.match(empty, /—/);
    assert.equal(empty.includes('/biblios/'), false);
  });
});
