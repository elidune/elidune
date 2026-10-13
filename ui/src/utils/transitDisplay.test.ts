import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { HoldDetailsStaff, Source } from '@/types';
import { shelfPatronLabel } from './holdPatron.ts';
import {
  canStartHoldTransit,
  transitListBarcode,
  transitListReader,
  transitListTitle,
} from './transitDisplay.ts';

const SITES: Source[] = [
  { id: 'site-main', key: 'main', name: 'Main' },
  { id: 'site-annex', key: 'annex', name: 'Annex' },
];

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
      items: [
        {
          id: 'i1',
          barcode: 'EX-1',
          callNumber: '800 PRI',
          borrowable: true,
          weedingStatus: 'onShelf',
          sourceName: 'Main',
          borrowed: false,
          availability: 'available',
          canHoldItem: true,
        },
      ],
    },
    user: {
      id: 'u1',
      firstname: 'Helene',
      lastname: 'Holder',
      login: 'helene',
      barcode: 'CARD-3',
    },
    createdAt: '2026-10-01T00:00:00Z',
    notifiedAt: null,
    expiresAt: null,
    status: 'pending',
    position: 1,
    notes: null,
    ...patch,
  };
}

describe('start transit when the copy is already at the pickup site', () => {
  it('hides the action when the copy site name is the pickup site', () => {
    assert.equal(canStartHoldTransit(hold(), null, SITES), false);
  });

  it('keeps the action when the copy is at another site', () => {
    const row = hold();
    row.biblio.items[0].sourceName = 'Annex';
    assert.equal(canStartHoldTransit(row, null, SITES), true);
  });

  it('keeps the action for a title-level hold until a copy is chosen', () => {
    assert.equal(canStartHoldTransit(hold({ itemId: null, biblio: { ...hold().biblio, items: [] } }), null, SITES), true);
  });

  it('does not guess when two sites share the copy site name', () => {
    const sites = [
      { id: 'a', key: 'a', name: 'Main' },
      { id: 'b', key: 'b', name: 'Main' },
    ];
    assert.equal(canStartHoldTransit(hold(), null, sites), true);
  });
});

describe('transit list fields already returned on the hold', () => {
  it('reads the barcode and the title, not the item id', () => {
    const row = hold();
    assert.equal(transitListBarcode(row), 'EX-1');
    assert.equal(transitListTitle(row), 'Privacy');
    assert.equal(transitListBarcode(row)?.includes(row.itemId ?? ''), false);
  });

  it('leaves barcode and title empty when the hold is not in the loaded page', () => {
    assert.equal(transitListBarcode(null), null);
    assert.equal(transitListTitle(undefined), null);
    assert.equal(transitListReader(null), null);
  });

  it('shows the card or initials and never the name', () => {
    const reader = transitListReader(hold());
    assert.equal(reader?.cardNumber, 'CARD-3');
    assert.equal(reader?.initials, 'HH');
    const label = shelfPatronLabel(reader, (key, values) =>
      key === 'holds.readerCardNo' ? `carte n° ${String(values?.number ?? '')}` : key,
    );
    assert.match(label, /CARD-3/);
    assert.equal(label.includes('Helene'), false);
    assert.equal(label.includes('Holder'), false);

    const initialsOnly = transitListReader(
      hold({
        user: { id: 'u1', firstname: 'Helene', lastname: 'Holder', barcode: null },
      }),
    );
    const initials = shelfPatronLabel(initialsOnly, (key) => key);
    assert.equal(initials, 'HH');
    assert.equal(initials.includes('Helene'), false);
  });
});

describe('transit list labels', () => {
  it('uses lecteur in French and adds the English keys', () => {
    assert.equal(fr.transits.columnReader, 'Lecteur');
    assert.equal(fr.transits.columnBarcode, 'Code-barres');
    assert.equal(fr.transits.columnTitle, 'Titre');
    assert.equal(/spécimen/i.test(fr.transits.columnReader), false);
    assert.equal(/spécimen/i.test(fr.transits.columnBarcode), false);
    assert.equal(/spécimen/i.test(fr.transits.columnTitle), false);
    assert.equal(en.transits.columnReader, 'Reader');
    assert.equal(en.transits.columnBarcode, 'Barcode');
    assert.equal(en.transits.columnTitle, 'Title');
    assert.equal(de.transits.columnReader, 'Leser');
    assert.equal(es.transits.columnReader, 'Lector');
  });
});
