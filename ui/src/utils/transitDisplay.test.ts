import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { HoldDetailsStaff, ItemTransit } from '@/types';
import { shelfPatronLabel } from './holdPatron.ts';
import {
  canStartHoldTransit,
  copyAlreadyAtPickupSite,
  transitListBarcode,
  transitListPatron,
  transitListTitle,
} from './transitDisplay.ts';

function hold(pickupSiteId: string | null = 'site-main'): HoldDetailsStaff {
  return {
    id: 'h1',
    userId: 'u1',
    biblioId: 'b1',
    itemId: 'i1',
    pickupSiteId,
    biblio: {
      id: 'b1',
      mediaType: 'printedText',
      isbn: null,
      title: 'From the hold',
      date: null,
      status: 0,
      isValid: true,
      archivedAt: null,
      weedingStatus: 'onShelf',
      author: null,
      items: [],
    },
    user: {
      id: 'u1',
      firstname: 'Helene',
      lastname: 'Holder',
      barcode: 'CARD-FROM-HOLD',
    },
    createdAt: '2026-10-01T00:00:00Z',
    notifiedAt: null,
    expiresAt: null,
    status: 'pending',
    position: 1,
    notes: null,
  };
}

function transit(patch: Partial<ItemTransit> = {}): ItemTransit {
  return {
    id: 't1',
    itemId: 'i1',
    holdId: 'h1',
    fromSourceId: 'site-annex',
    toSourceId: 'site-main',
    status: 'requested',
    createdAt: '2026-10-01T00:00:00Z',
    barcode: 'EX-1',
    title: 'Privacy',
    sourceId: 'site-annex',
    sourceName: 'Annex',
    patron: { cardNumber: 'CARD-3', initials: 'HH' },
    ...patch,
  };
}

describe('start transit when the copy site id equals the pickup site', () => {
  it('hides the action when sourceId is the pickup site', () => {
    assert.equal(copyAlreadyAtPickupSite('site-main', 'site-main'), true);
    assert.equal(canStartHoldTransit(hold(), null, 'site-main'), false);
    assert.equal(canStartHoldTransit(hold(), transit({ sourceId: 'site-main', status: 'received' })), false);
  });

  it('keeps the action when sourceId is another site', () => {
    assert.equal(canStartHoldTransit(hold(), null, 'site-annex'), true);
  });

  it('does not guess from a site name when sourceId is missing', () => {
    assert.equal(canStartHoldTransit(hold(), transit({ sourceId: undefined, sourceName: 'Main', status: 'received' })), true);
    assert.equal(copyAlreadyAtPickupSite('site-main', null), false);
  });
});

describe('transit list fields on the transit', () => {
  it('reads barcode and title from the transit, not the hold', () => {
    const row = transit();
    assert.equal(transitListBarcode(row), 'EX-1');
    assert.equal(transitListTitle(row), 'Privacy');
    assert.notEqual(transitListBarcode(row), row.itemId);
  });

  it('leaves barcode, title, and patron empty when the server omits them', () => {
    const row = transit({ barcode: undefined, title: null, patron: null });
    assert.equal(transitListBarcode(row), null);
    assert.equal(transitListTitle(row), null);
    assert.equal(transitListPatron(row), null);
  });

  it('shows the card or initials and never a name', () => {
    const patron = transitListPatron(transit());
    const label = shelfPatronLabel(patron, (key, values) =>
      key === 'holds.readerCardNo' ? `carte n° ${String(values?.number ?? '')}` : key,
    );
    assert.match(label, /CARD-3/);
    assert.equal(label.includes('Helene'), false);
    assert.equal(label.includes('Holder'), false);

    const initials = transitListPatron(transit({ patron: { cardNumber: null, initials: 'H.H' } }));
    assert.equal(shelfPatronLabel(initials, (key) => key), 'HH');

    const named = transitListPatron(transit({ patron: { cardNumber: null, initials: 'Helene Holder' } }));
    assert.equal(named, null);
  });
});

describe('transit list labels', () => {
  it('uses lecteur in French and adds the English keys', () => {
    assert.equal(fr.transits.columnReader, 'Lecteur');
    assert.equal(fr.transits.columnBarcode, 'Code-barres');
    assert.equal(fr.transits.columnTitle, 'Titre');
    assert.equal(/spécimen/i.test(fr.transits.columnReader), false);
    assert.equal(en.transits.columnReader, 'Reader');
    assert.equal(en.transits.columnBarcode, 'Barcode');
    assert.equal(en.transits.columnTitle, 'Title');
    assert.equal(de.transits.columnReader, 'Leser');
    assert.equal(es.transits.columnReader, 'Lector');
  });
});
