import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { HoldDetailsStaff, ItemTransit } from '@/types';
import {
  canStartHoldTransit,
  copyAlreadyAtPickupSite,
  sameSiteSetAsideBarcode,
  transitListBarcode,
  transitListPatron,
  transitListTitle,
  transitPatronLabel,
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

describe('set aside after transit_same_site', () => {
  it('offers the copy barcode already on screen and nothing else', () => {
    const copies = [{ id: 'i1', barcode: ' EX-9 ' }, { id: 'i2', barcode: 'EX-2' }];
    assert.equal(sameSiteSetAsideBarcode('transit_same_site', copies, 'i1'), 'EX-9');
    assert.equal(sameSiteSetAsideBarcode('transit_same_site', copies, 'missing'), null);
    assert.equal(sameSiteSetAsideBarcode('transit_same_site', [{ id: 'i1', barcode: '  ' }], 'i1'), null);
    assert.equal(sameSiteSetAsideBarcode('business_rule_violation', copies, 'i1'), null);
    assert.equal(sameSiteSetAsideBarcode('transit_same_site', copies, null), null);
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

  it('shows the card and initials together and never a name', () => {
    const phrase = (number: string) => `carte n° ${number}`;
    const label = transitPatronLabel(transitListPatron(transit()), phrase);
    assert.equal(label, 'carte n° CARD-3 HH');
    assert.equal(label?.includes('Helene'), false);
    assert.equal(label?.includes('Holder'), false);

    const dotted = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: 'CARD-3', initials: 'H.H.' } })),
      phrase,
    );
    assert.equal(dotted, 'carte n° CARD-3 HH');

    const initials = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: null, initials: 'H.H' } })),
      phrase,
    );
    assert.equal(initials, 'HH');

    const cardOnly = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: 'CARD-3', initials: 'Helene Holder' } })),
      phrase,
    );
    assert.equal(cardOnly, 'carte n° CARD-3');

    const named = transitListPatron(transit({ patron: { cardNumber: null, initials: 'Helene Holder' } }));
    assert.equal(named, null);
    assert.equal(transitPatronLabel(named, phrase), null);
    assert.equal(transitPatronLabel(null, phrase), null);
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
