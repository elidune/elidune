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
    fromSiteId: 'site-annex',
    toSiteId: 'site-main',
    status: 'requested',
    createdAt: '2026-10-01T00:00:00Z',
    barcode: 'EX-1',
    title: 'Privacy',
    siteId: 'site-annex',
    siteName: 'Annex',
    patron: { cardNumber: 'CARD-3', initials: 'HH' },
    ...patch,
  };
}

describe('start transit when the copy site id equals the pickup site', () => {
  it('hides the action when siteId is the pickup site', () => {
    assert.equal(copyAlreadyAtPickupSite('site-main', 'site-main'), true);
    assert.equal(canStartHoldTransit(hold(), null, 'site-main'), false);
    assert.equal(canStartHoldTransit(hold(), transit({ siteId: 'site-main', status: 'received' })), false);
  });

  it('keeps the action when siteId is another site', () => {
    assert.equal(canStartHoldTransit(hold(), null, 'site-annex'), true);
  });

  it('does not guess from a site name when siteId is missing', () => {
    assert.equal(canStartHoldTransit(hold(), transit({ siteId: undefined, siteName: 'Main', status: 'received' })), true);
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
    assert.equal(dotted, 'carte n° CARD-3 H.H.');

    const spaced = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: 'CARD-3', initials: 'J. D.' } })),
      phrase,
    );
    assert.equal(spaced, 'carte n° CARD-3 J. D.');

    const initials = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: null, initials: 'H.H' } })),
      phrase,
    );
    assert.equal(initials, 'H.H');

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

  it('reads a transit-list payload the way the list route serializes it', () => {
    const phrase = (number: string) => `carte n° ${number}`;
    const serverInitials = JSON.parse(`{
      "id": "9001",
      "itemId": "8001",
      "holdId": "7001",
      "barcode": "EX-1",
      "title": "Privacy",
      "siteId": "10",
      "siteName": "Annex",
      "patron": { "cardNumber": "CARD-42", "initials": "JD" },
      "fromSiteId": "11",
      "toSiteId": "10",
      "status": "inTransit",
      "notes": null,
      "createdAt": "2026-10-14T08:00:00Z",
      "shippedAt": "2026-10-14T09:00:00Z",
      "receivedAt": null,
      "cancelledAt": null,
      "shippedBy": "1",
      "receivedBy": null,
      "cancelledBy": null,
      "reversedFromId": null
    }`) as ItemTransit;
    assert.equal(transitListBarcode(serverInitials), 'EX-1');
    assert.equal(transitListTitle(serverInitials), 'Privacy');
    assert.equal(transitPatronLabel(transitListPatron(serverInitials), phrase), 'carte n° CARD-42 JD');

    const spacedInitials = JSON.parse(`{
      "id": "9002",
      "itemId": "8001",
      "holdId": "7001",
      "barcode": "EX-2",
      "title": "Privacy",
      "siteId": "10",
      "siteName": "Annex",
      "patron": {
        "cardNumber": "CARD-42",
        "initials": "J. D.",
        "firstname": "Jean",
        "lastname": "Dupont"
      },
      "fromSiteId": "11",
      "toSiteId": "10",
      "status": "requested",
      "createdAt": "2026-10-14T08:00:00Z"
    }`) as ItemTransit;
    const spacedLabel = transitPatronLabel(transitListPatron(spacedInitials), phrase);
    assert.equal(spacedLabel, 'carte n° CARD-42 J. D.');
    assert.equal(spacedLabel?.includes('Jean'), false);
    assert.equal(spacedLabel?.includes('Dupont'), false);

    const accented = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: 'CARD-42', initials: 'ÉZ' } })),
      phrase,
    );
    assert.equal(accented, 'carte n° CARD-42 ÉZ');

    const blankInitials = transitPatronLabel(
      transitListPatron(transit({ patron: { cardNumber: 'CARD-42', initials: '' } })),
      phrase,
    );
    assert.equal(blankInitials, 'carte n° CARD-42');

    const closedHold = JSON.parse(`{
      "id": "9003",
      "itemId": "8002",
      "holdId": "7002",
      "barcode": "RC-B5TE-101229",
      "title": "Return copy",
      "siteId": "10",
      "siteName": "Annex",
      "patron": null,
      "fromSiteId": "11",
      "toSiteId": "10",
      "status": "inTransit",
      "createdAt": "2026-10-14T08:00:00Z"
    }`) as ItemTransit;
    assert.equal(transitListBarcode(closedHold), 'RC-B5TE-101229');
    assert.equal(transitListPatron(closedHold), null);
    assert.equal(transitPatronLabel(transitListPatron(closedHold), phrase), null);

    const returnHome = JSON.parse(`{
      "id": "9004",
      "itemId": "8002",
      "holdId": null,
      "barcode": "RC-B5TE-101229",
      "title": "Return copy",
      "patron": null,
      "fromSiteId": "10",
      "toSiteId": "11",
      "status": "requested",
      "createdAt": "2026-10-14T08:00:00Z",
      "reversedFromId": "9003"
    }`) as ItemTransit;
    assert.equal(transitListPatron(returnHome), null);
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
