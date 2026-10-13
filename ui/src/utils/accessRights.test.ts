import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import OverdueBorrowerSummary from '@/components/loans/OverdueBorrowerSummary';
import { ownContactOnly } from '@/types';
import { ownContactPayload } from '@/utils/ownContactForm';
import { borrowerShownToAgent } from '@/utils/readerIdentifier';
import { staffAccess } from '@/utils/staffAccess';

function token(rights: Record<string, string>): string {
  const payload = Buffer.from(JSON.stringify({ rights })).toString('base64url');
  return `e30.${payload}.sig`;
}

const READER = token({
  itemsRights: 'Read',
  usersRights: 'Own',
  loansRights: 'Own',
  holdsRights: 'Own',
  settingsRights: 'Read',
  eventsRights: 'Read',
});

const LOANS_ONLY = token({
  itemsRights: 'Read',
  usersRights: 'None',
  loansRights: 'Read',
  holdsRights: 'None',
  settingsRights: 'None',
  eventsRights: 'None',
});

const CATALOGER = token({
  itemsRights: 'Write',
  usersRights: 'None',
  loansRights: 'None',
  holdsRights: 'None',
  settingsRights: 'Read',
  weedingRights: 'Read',
});

describe('staff menus follow rights', () => {
  it('keeps a reader on the public shell', () => {
    const access = staffAccess(null, READER);
    assert.equal(access.internalCatalog, false);
    assert.equal(access.loansRead, false);
    assert.equal(access.holdsStaff, false);
    assert.equal(access.usersRead, false);
    assert.equal(access.itemsWrite, false);
    assert.equal(access.stats, false);
    assert.equal(access.fullSettings, false);
    assert.equal(access.librarySettings, false);
    assert.equal(ownContactOnly(null, READER), true);
  });

  it('opens the desk for loans read and hides reader records', () => {
    const access = staffAccess(null, LOANS_ONLY);
    assert.equal(access.internalCatalog, true);
    assert.equal(access.loansRead, true);
    assert.equal(access.usersRead, false);
    assert.equal(access.holdsStaff, false);
    assert.equal(access.itemsWrite, false);
    assert.equal(access.homeStats, true);
    assert.equal(ownContactOnly(null, LOANS_ONLY), true);
  });

  it('opens cataloging and weeding for items write without the circulation desk', () => {
    const access = staffAccess(null, CATALOGER);
    assert.equal(access.internalCatalog, true);
    assert.equal(access.itemsWrite, true);
    assert.equal(access.weeding, true);
    assert.equal(access.loansRead, false);
    assert.equal(access.librarySettings, true);
    assert.equal(access.fullSettings, false);
  });
});

describe('own account fields', () => {
  it('sends only the contact fields the server accepts', () => {
    const body = ownContactPayload(
      {
        email: ' ada@library.test ',
        phone: '',
        addrStreet: '1 rue',
        addrZipCode: '75001',
        addrCity: 'Paris',
        eventsConsent: true,
        receiveReminders: false,
      },
      { includeEventsConsent: true, includeReminders: true },
    );
    assert.deepEqual(Object.keys(body).sort(), [
      'addrCity',
      'addrStreet',
      'addrZipCode',
      'email',
      'eventsConsent',
      'phone',
      'receiveReminders',
    ]);
    assert.equal(body.email, 'ada@library.test');
    assert.equal(body.phone, null);
    assert.equal(body.addrZipCode, 75001);
    assert.equal('firstname' in body, false);
    assert.equal('login' in body, false);
    assert.equal('barcode' in body, false);
  });
});

describe('reader identity without users read', () => {
  it('keeps the card and server initials and drops the name', () => {
    const shown = borrowerShownToAgent(
      {
        id: '7',
        firstname: 'Marie',
        lastname: 'Curie',
        email: 'marie@library.test',
        phone: '0600000000',
        barcode: 'L-10042',
        initials: 'MC',
      },
      false,
    );
    assert.deepEqual(shown, { id: '7', cardNumber: 'L-10042', initials: 'MC' });
  });

  it('does not render the name on the overdue summary', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'fr',
      resources: { fr: { translation: fr } },
      interpolation: { escapeValue: false },
    });
    const html = renderToStaticMarkup(
      createElement(
        I18nextProvider,
        { i18n },
        createElement(
          MemoryRouter,
          null,
          createElement(OverdueBorrowerSummary, {
            user: {
              id: '7',
              firstname: 'Marie',
              lastname: 'Curie',
              email: 'marie@library.test',
              phone: '0600000000',
              barcode: 'L-10042',
            },
            count: 1,
            linked: false,
          }),
        ),
      ) as ReactElement,
    );
    assert.equal(html.includes('Marie'), false);
    assert.equal(html.includes('Curie'), false);
    assert.equal(html.includes('marie@library.test'), false);
    assert.equal(html.includes('0600000000'), false);
    assert.equal(html.includes('L-10042'), true);
  });
});

describe('reader and copy vocabulary', () => {
  it('uses lecteur and exemplaire in French and avoids spécimen', () => {
    const catalogs = { fr, en, de, es };
    const walk = (value: unknown, hit: (text: string) => void) => {
      if (typeof value === 'string') hit(value);
      else if (Array.isArray(value)) value.forEach((item) => walk(item, hit));
      else if (value && typeof value === 'object') {
        Object.values(value).forEach((item) => walk(item, hit));
      }
    };
    const french: string[] = [];
    walk(fr, (text) => french.push(text));
    const blob = french.join('\n');
    assert.equal(/usager/i.test(blob), false);
    assert.equal(/spécimen/i.test(blob), false);
    assert.equal(/\bspecimen\b/i.test(blob), false);
    assert.equal(fr.nav.users, 'Lecteurs');
    assert.equal(fr.nav.sectionPatron, 'Lecteur');
    assert.match(fr.items.specimens, /Exemplaire/);
    assert.equal(en.nav.users, 'Readers');
    assert.equal(de.nav.users, 'Leser');
    assert.equal(es.nav.users, 'Lectores');
    for (const catalog of Object.values(catalogs)) {
      const values: string[] = [];
      walk(catalog, (text) => values.push(text));
      assert.equal(values.some((text) => /\bspécimen\b/i.test(text)), false);
    }
  });
});
