import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import ToPickForceLoanDialog from './ToPickForceLoanDialog.tsx';
import {
  readItemToPickForOther,
  toPickForceFromDeskCopy,
  toPickForceFromError,
} from '@/utils/toPickDesk';

const NAME = 'Helene Holder';
const CARD = 'CARD-3';

const catalogs = { fr, en, de, es } as const;
type Lang = keyof typeof catalogs;

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

function renderDialog(
  i18n: I18nInstance,
  props: { patronName?: string | null; cardNumber?: string | null; initials?: string | null },
): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(ToPickForceLoanDialog, {
        isOpen: true,
        patronName: props.patronName ?? null,
        cardNumber: props.cardNumber ?? null,
        initials: props.initials ?? null,
        onCancel: () => undefined,
        onForce: () => undefined,
      }),
    ) as ReactElement,
  );
}

describe('to-pick force dialog', () => {
  it('shows the wrapping title and the force action in every locale', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const title = catalogs[lang].loans.toPickForOtherTitle;
      const force = catalogs[lang].loans.forceLoan;
      assert.equal(title.includes('{{'), false, lang);
      assert.equal(force.length > 0, true, lang);
      const html = renderDialog(await i18nFor(lang), { cardNumber: CARD, initials: 'HH' });
      assert.equal(html.includes(title), true, lang);
      assert.equal(html.includes(force), true, lang);
      assert.match(html, /<h2[^>]*class="[^"]*break-words[^"]*"/);
      assert.equal(/<h2[^>]*truncate/.test(html), false);
      assert.equal(html.includes('scanAlert'), false);
    }
    assert.equal(catalogs.fr.loans.toPickForOtherTitle, 'Exemplaire en attente pour un autre usager');
  });

  it('shows the full name when holdFor is a users-read profile', async () => {
    const identity = toPickForceFromDeskCopy(
      {
        availability: 'onHold',
        holdFor: {
          id: '3',
          firstname: 'Helene',
          lastname: 'Holder',
          login: 'helene',
          barcode: CARD,
        },
      },
      '2',
    );
    assert.equal(identity?.patronName, NAME);
    assert.equal(identity?.cardNumber, null);
    const html = renderDialog(await i18nFor('fr'), identity ?? {});
    assert.equal(html.includes(NAME), true);
    assert.equal(html.includes(CARD), false);
    assert.equal(html.includes('Exemplaire en attente pour un autre usager'), true);
    assert.equal(html.includes('Forcer le prêt'), true);
  });

  it('shows the card or initials when holdFor is minimized', async () => {
    const identity = toPickForceFromDeskCopy(
      {
        availability: 'onHold',
        holdFor: { id: '3', cardNumber: CARD, initials: 'HH' },
      },
      '2',
    );
    assert.equal(identity?.patronName, null);
    assert.equal(identity?.cardNumber, CARD);
    assert.equal(identity?.initials, 'HH');
    const html = renderDialog(await i18nFor('fr'), identity ?? {});
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes(CARD), true);
    assert.equal(html.includes('Forcer le prêt'), true);

    const initialsOnly = toPickForceFromDeskCopy(
      {
        availability: 'onHold',
        holdFor: { id: '3', cardNumber: null, initials: 'HH' },
      },
      '2',
    );
    assert.equal(initialsOnly?.cardNumber, null);
    assert.equal(initialsOnly?.initials, 'HH');
    const initialsHtml = renderDialog(await i18nFor('en'), initialsOnly ?? {});
    assert.equal(initialsHtml.includes('HH'), true);
    assert.equal(initialsHtml.includes('Helene'), false);
  });

  it('reads the 422 details card and initials and keeps a full holdFor name', () => {
    const error = {
      isAxiosError: true,
      response: {
        status: 422,
        data: {
          code: 'item_to_pick_for_other',
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
    };
    const parsed = readItemToPickForOther(error);
    assert.deepEqual(parsed, {
      holdId: '11',
      userId: '3',
      cardNumber: CARD,
      initials: 'HH',
    });
    assert.equal(JSON.stringify(parsed).includes(NAME), false);

    const fromDetails = toPickForceFromError(error, null);
    assert.equal(fromDetails?.patronName, null);
    assert.equal(fromDetails?.cardNumber, CARD);
    assert.equal(fromDetails?.initials, 'HH');

    const withProfile = toPickForceFromError(error, {
      id: '3',
      firstname: 'Helene',
      lastname: 'Holder',
      barcode: CARD,
    });
    assert.equal(withProfile?.patronName, NAME);
    assert.equal(withProfile?.cardNumber, null);

    assert.equal(toPickForceFromError({ isAxiosError: true, response: { data: { code: 'card_lost' } } }, null), null);
    assert.equal(
      toPickForceFromDeskCopy({ availability: 'onLoan', holdFor: { id: '9', cardNumber: CARD, initials: 'HH' } }, '2'),
      null,
    );
    assert.equal(
      toPickForceFromDeskCopy(
        { availability: 'onHold', holdFor: { id: '2', cardNumber: CARD, initials: 'HH' } },
        '2',
      ),
      null,
    );
  });
});
