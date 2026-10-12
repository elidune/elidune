import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import { canReadUsers } from '@/types';
import { otherBorrowerFromPayload } from '@/utils/deskScanAlert';
import {
  ITEM_ON_LOAN_TO_OTHER_BORROWER,
  checkoutNoticeForConflict,
} from '@/utils/loanConflict';
import DeskScanNotice from './DeskScanNotice.tsx';

const FULL_NAME = 'Julien Dubois';
const CARD = 'L-10042';
const USER_ID = '42';

type Lang = 'fr' | 'en' | 'de' | 'es';

function accessToken(usersRights: string): string {
  const payload = Buffer.from(JSON.stringify({ rights: { usersRights } })).toString('base64url');
  return `e30.${payload}.sig`;
}

async function i18nFor(lng: Lang): Promise<I18nInstance> {
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

function renderNotice(
  i18n: I18nInstance,
  canReadBorrowers: boolean,
  alert: {
    kind: 'onLoan';
    patronName: string | null;
    otherBorrower?: {
      userId: string | null;
      cardNumber: string | null;
      initials: string | null;
    } | null;
  },
): string {
  const tree = createElement(
    I18nextProvider,
    { i18n },
    createElement(
      MemoryRouter,
      null,
      createElement(DeskScanNotice, {
        alert,
        canReadBorrowers,
      }),
    ),
  ) as ReactElement;
  return renderToStaticMarkup(tree);
}

function conflict409(snapshot: Record<string, unknown>) {
  return checkoutNoticeForConflict(
    {
      response: {
        status: 409,
        data: {
          code: ITEM_ON_LOAN_TO_OTHER_BORROWER,
          error: 'Conflict',
          message: 'This item is on loan to another borrower.',
        },
      },
    },
    {
      patronId: '1',
      snapshot: {
        availability: 'onLoan',
        loanId: 'loan-other',
        loanUserId: USER_ID,
        loanUserName: FULL_NAME,
        ...snapshot,
      },
    },
  );
}

describe('desk 409 notice hides the other borrower name', () => {
  it('links the card number when staff can read borrowers, and never prints the name', async () => {
    assert.equal(canReadUsers(null, accessToken('read')), true);
    assert.equal(canReadUsers({ rights: { usersRights: 'write' } }, null), true);
    const i18n = await i18nFor('fr');
    const html = renderNotice(i18n, canReadUsers(null, accessToken('read')), {
      kind: 'onLoan',
      patronName: FULL_NAME,
      otherBorrower: { userId: USER_ID, cardNumber: CARD, initials: 'JD' },
    });
    assert.equal(html.includes(FULL_NAME), false);
    assert.equal(html.includes('Julien'), false);
    assert.equal(html.includes('Dubois'), false);
    assert.match(html, new RegExp(`<a[^>]*href="/users/${USER_ID}"[^>]*>${CARD}</a>`));
    assert.match(html, /exemplaire/);
    assert.match(html, /lecteur/i);
    assert.match(html, /carte n°/);
    assert.equal(html.includes('code-barres'), false);
  });

  it('shows the card number as plain text without read permission', async () => {
    assert.equal(canReadUsers(null, accessToken('none')), false);
    assert.equal(canReadUsers(null, accessToken('own')), false);
    assert.equal(canReadUsers(null, null), false);
    const i18n = await i18nFor('fr');
    const html = renderNotice(i18n, canReadUsers(null, accessToken('none')), {
      kind: 'onLoan',
      patronName: FULL_NAME,
      otherBorrower: { userId: USER_ID, cardNumber: CARD, initials: 'JD' },
    });
    assert.equal(html.includes('<a'), false);
    assert.equal(html.includes('href='), false);
    assert.equal(html.includes(CARD), true);
    assert.equal(html.includes(FULL_NAME), false);
    assert.equal(html.includes('Julien'), false);
    assert.equal(html.includes('Dubois'), false);
  });

  it('renders server initials as plain text without borrower read permission', async () => {
    const ref = otherBorrowerFromPayload({
      id: USER_ID,
      cardNumber: null,
      initials: 'J.D.',
      firstname: 'Julien',
      lastname: 'Dubois',
    });
    assert.deepEqual(ref, { userId: USER_ID, cardNumber: null, initials: 'JD' });
    const i18n = await i18nFor('fr');
    const html = renderNotice(i18n, canReadUsers(null, accessToken('own')), {
      kind: 'onLoan',
      patronName: FULL_NAME,
      otherBorrower: ref,
    });
    assert.equal(html.includes('<a'), false);
    assert.equal(html.includes('href='), false);
    assert.equal(html.includes('JD'), true);
    assert.equal(html.includes('J.D.'), false);
    assert.equal(html.includes(FULL_NAME), false);
    assert.equal(html.includes('Julien'), false);
    assert.equal(html.includes('Dubois'), false);
    assert.equal(html.includes('code-barres'), false);
  });

  it('falls back to initials and still hides the name', async () => {
    const i18n = await i18nFor('fr');
    const html = renderNotice(i18n, true, {
      kind: 'onLoan',
      patronName: FULL_NAME,
      otherBorrower: { userId: USER_ID, cardNumber: null, initials: 'JD' },
    });
    assert.match(html, new RegExp(`<a[^>]*href="/users/${USER_ID}"[^>]*>JD</a>`));
    assert.equal(html.includes(CARD), false);
    assert.equal(html.includes(FULL_NAME), false);
    assert.equal(html.includes('Julien'), false);
    assert.equal(html.includes('Dubois'), false);
    assert.equal(html.includes('code-barres'), false);
  });

  it('keeps the name out of every locale, and out of the 409 notice payload', async () => {
    const cardWording = {
      fr: 'carte n°',
      en: 'library card no.',
      de: 'Ausweis-Nr.',
      es: 'carné n.º',
    } as const;
    for (const lng of ['fr', 'en', 'de', 'es'] as const satisfies readonly Lang[]) {
      const html = renderNotice(await i18nFor(lng), true, {
        kind: 'onLoan',
        patronName: FULL_NAME,
        otherBorrower: { userId: USER_ID, cardNumber: CARD, initials: 'JD' },
      });
      assert.equal(html.includes(FULL_NAME), false, lng);
      assert.equal(html.includes('Julien'), false, lng);
      assert.equal(html.includes(CARD), true, lng);
      assert.equal(html.includes(cardWording[lng]), true, lng);
      assert.equal(/code-barres|barcode|código de barras/i.test(html), false, lng);
    }

    const withCard = conflict409({ loanUserCard: CARD });
    const cardBlob = JSON.stringify(withCard);
    assert.equal(cardBlob.includes(FULL_NAME), false);
    assert.equal(cardBlob.includes('Julien'), false);
    assert.equal(withCard?.patronName, null);
    assert.equal(withCard?.otherBorrower?.cardNumber, CARD);
    assert.equal(withCard?.otherBorrower?.userId, USER_ID);

    const initialsOnly = conflict409({});
    assert.equal(JSON.stringify(initialsOnly).includes(FULL_NAME), false);
    assert.equal(initialsOnly?.patronName, null);
    assert.equal(initialsOnly?.otherBorrower?.cardNumber ?? null, null);
    assert.equal(initialsOnly?.otherBorrower?.initials, 'JD');
  });
});
