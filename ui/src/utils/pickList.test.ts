import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { shelfPatronLabel, toPickPatronPhrase } from './holdPatron.ts';
import {
  pickListRequestSiteId,
  readPickListSiteChoice,
  resolvePickListSiteId,
  writePickListSiteChoice,
  normalizePickListRow,
} from './pickList.ts';
import { readItemToPickForOther } from './toPickDesk.ts';

const NAME = 'Helene Holder';
const CARD = 'CARD-3';

function translate(catalog: { holds: { readerCardNo: string }; loans: { pickPatronUnknown: string } }) {
  return (key: string, values?: Record<string, unknown>) => {
    const template =
      key === 'holds.readerCardNo'
        ? catalog.holds.readerCardNo
        : key === 'loans.pickPatronUnknown'
          ? catalog.loans.pickPatronUnknown
          : key;
    return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) => String(values?.[name] ?? ''));
  };
}

function memoryStorage(initial?: Record<string, string>) {
  const data = new Map(Object.entries(initial ?? {}));
  return {
    getItem(key: string) {
      return data.has(key) ? data.get(key)! : null;
    },
    setItem(key: string, value: string) {
      data.set(key, value);
    },
  };
}

const SITES = [
  { id: '1', default: true, name: 'Site 1' },
  { id: '2', default: false, name: 'Annexe' },
];

describe('pick list site filter', () => {
  it('opens on all sites and omits siteId even when a default site exists', () => {
    const siteId = resolvePickListSiteId(SITES, null);
    assert.equal(siteId, '');
    assert.equal(pickListRequestSiteId(siteId), undefined);
    assert.equal(resolvePickListSiteId(SITES, ''), '');
  });

  it('remembers an explicit site and drops one that is no longer in the list', () => {
    const storage = memoryStorage();
    assert.equal(readPickListSiteChoice(storage), null);

    writePickListSiteChoice(storage, '2');
    assert.equal(resolvePickListSiteId(SITES, readPickListSiteChoice(storage)), '2');
    assert.equal(pickListRequestSiteId('2'), '2');

    writePickListSiteChoice(storage, '');
    assert.equal(readPickListSiteChoice(storage), '');
    assert.equal(pickListRequestSiteId(resolvePickListSiteId(SITES, '')), undefined);

    writePickListSiteChoice(storage, 'gone');
    assert.equal(resolvePickListSiteId(SITES, readPickListSiteChoice(storage)), '');
    assert.equal(pickListRequestSiteId('   '), undefined);
  });
});

describe('pick list shelf identity', () => {
  it('keeps card and initials and drops every name field', () => {
    const row = normalizePickListRow({
      holdId: '10',
      itemId: '20',
      title: 'Privacy',
      userName: NAME,
      patronFirstname: 'Helene',
      patronLastname: 'Holder',
      holdFor: {
        id: '3',
        cardNumber: CARD,
        initials: 'HH',
        firstname: 'Helene',
        lastname: 'Holder',
        login: 'helene',
        email: 'helene.holder@secret.test',
      },
      pickupDeadline: '2026-10-20T18:00:00Z',
      pickDueAt: '2026-10-13T00:00:00Z',
      overdue: false,
    });
    const blob = JSON.stringify(row);
    assert.equal(blob.includes('Helene'), false);
    assert.equal(blob.includes('Holder'), false);
    assert.equal(blob.includes('helene'), false);
    assert.equal(blob.includes('secret'), false);
    assert.equal(blob.includes(NAME), false);
    assert.deepEqual(row?.holdFor, { id: '3', cardNumber: CARD, initials: 'HH' });
    assert.equal(row?.userId, '3');
    assert.equal(row?.pickupDeadline, '2026-10-20T18:00:00Z');
    assert.equal('userName' in (row ?? {}), false);
  });

  it('turns a full profile into card or initials and never stores the name', () => {
    const named = normalizePickListRow({
      holdId: '1',
      itemId: '2',
      holdFor: {
        id: '9',
        barcode: CARD,
        firstname: 'Helene',
        lastname: 'Holder',
        login: 'helene',
      },
    });
    assert.equal(JSON.stringify(named).includes('Helene'), false);
    assert.equal(named?.holdFor.cardNumber, CARD);
    assert.equal(named?.holdFor.initials, 'HH');

    const initialsOnly = normalizePickListRow({
      holdId: '1',
      itemId: '2',
      userId: '9',
      userName: NAME,
      holdFor: { id: '9', firstname: 'Helene', lastname: 'Holder' },
    });
    assert.equal(JSON.stringify(initialsOnly).includes('Helene'), false);
    assert.equal(initialsOnly?.holdFor.cardNumber, null);
    assert.equal(initialsOnly?.holdFor.initials, 'HH');
  });

  it('prints the card phrase or initials in every locale, never the name', () => {
    const labels = {
      fr: 'carte n°',
      en: 'card number',
      de: 'Leseausweis-Nr.',
      es: 'n.º de carné',
    } as const;
    const catalogs = { fr, en, de, es };
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const t = translate(catalogs[lang]);
      const withCard = shelfPatronLabel(
        { id: '3', cardNumber: CARD, initials: 'HH', firstname: 'Helene', lastname: 'Holder' },
        t,
      );
      assert.equal(withCard.includes(labels[lang]), true, lang);
      assert.equal(withCard.includes(CARD), true, lang);
      assert.equal(withCard.includes('Helene'), false, lang);
      assert.equal(withCard.includes('Holder'), false, lang);

      const initials = shelfPatronLabel(
        { id: '3', cardNumber: null, initials: 'HH', firstname: 'Helene', lastname: 'Holder' },
        t,
      );
      assert.equal(initials, 'HH', lang);
      assert.equal(initials.includes('Helene'), false, lang);
    }
  });
});

describe('POST /loans 422 item_to_pick_for_other', () => {
  it('reads details.cardNumber and initials and ignores a name', () => {
    const parsed = readItemToPickForOther({
      isAxiosError: true,
      response: {
        status: 422,
        data: {
          code: 'item_to_pick_for_other',
          error: 'Business Rule Violation',
          message: 'This copy is on the pick list for another patron — use force=true to override',
          userName: NAME,
          details: {
            holdId: '11',
            userId: '3',
            cardNumber: CARD,
            initials: 'HH',
            firstname: 'Helene',
            lastname: 'Holder',
          },
        },
      },
    });
    assert.deepEqual(parsed, {
      holdId: '11',
      userId: '3',
      cardNumber: CARD,
      initials: 'HH',
    });
    assert.equal(JSON.stringify(parsed).includes('Helene'), false);
    assert.equal(JSON.stringify(parsed).includes(NAME), false);

    const phrase = toPickPatronPhrase(parsed, translate(fr));
    assert.equal(phrase, `carte n° ${CARD}`);
    assert.equal(phrase.includes('Helene'), false);
    assert.equal(toPickPatronPhrase({ cardNumber: null, initials: 'H.H.' }, translate(en)), 'HH');
    assert.equal(toPickPatronPhrase({ cardNumber: null, initials: null }, translate(es)), 'otro lector');
  });
});
