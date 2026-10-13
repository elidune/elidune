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
import HoldPatronLabel from '../holds/HoldPatronLabel.tsx';
import HoldShelfSlip from '../holds/HoldShelfSlip.tsx';
import ItemAvailabilityBadge, { ItemHoldPatronLine } from './ItemAvailabilityBadge.tsx';
import NoticeHoldsWaiting from '../holds/NoticeHoldsWaiting.tsx';

const FULL = {
  id: '3',
  firstname: 'Helene',
  lastname: 'Holder',
  login: 'helene',
  barcode: 'CARD-3',
};

const MINIMIZED = {
  id: '3',
  cardNumber: 'CARD-3',
  initials: 'HH',
};

const CARD_LABEL = {
  fr: 'carte n°',
  en: 'card number',
  de: 'Leseausweis-Nr.',
  es: 'n.º de carné',
} as const;

async function i18nFor(lng: keyof typeof CARD_LABEL): Promise<I18nInstance> {
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

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement(MemoryRouter, null, node)) as ReactElement,
  );
}

describe('hold patron display', () => {
  it('shows Réservé and the waiting count to readers, never holdFor', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(
        'div',
        null,
        createElement(ItemAvailabilityBadge, { availability: 'onHold', audience: 'public' }),
        createElement(ItemHoldPatronLine, {
          audience: 'public',
          availability: 'onHold',
          holdFor: FULL,
          linked: true,
        }),
        createElement(NoticeHoldsWaiting, { holdsCount: 2 }),
      ),
    );
    assert.match(html, /Réservé/);
    assert.match(html, /Réservation/);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('CARD-3'), false);
    assert.equal(html.includes('holdFor'), false);
  });

  it('shows the name on a staff notice only for the full profile', async () => {
    const i18n = await i18nFor('fr');
    const full = render(
      i18n,
      createElement(ItemHoldPatronLine, {
        audience: 'staff',
        availability: 'onHold',
        holdFor: FULL,
        linked: true,
      }),
    );
    assert.match(full, /Réservé pour/);
    assert.match(full, /Helene Holder/);
    assert.match(full, /href="\/users\/3"/);

    const minimized = render(
      i18n,
      createElement(ItemHoldPatronLine, {
        audience: 'staff',
        availability: 'onHold',
        holdFor: MINIMIZED,
        linked: false,
      }),
    );
    assert.match(minimized, /carte n°/);
    assert.match(minimized, /CARD-3/);
    assert.equal(minimized.includes('Helene'), false);
    assert.equal(minimized.includes('<a'), false);
  });

  it('uses the hold card label in every locale on the hold list', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const html = render(
        await i18nFor(lang),
        createElement(HoldPatronLabel, { user: MINIMIZED, linked: false }),
      );
      assert.equal(html.includes(CARD_LABEL[lang]), true, lang);
      assert.equal(html.includes('CARD-3'), true, lang);
      assert.equal(html.includes('Helene'), false, lang);
      assert.equal(html.includes('library card no.'), false, lang);
    }
    const named = render(await i18nFor('en'), createElement(HoldPatronLabel, { user: FULL, linked: true }));
    assert.match(named, /Helene Holder/);
    assert.match(named, /href="\/users\/3"/);
  });

  it('prints the slip with card or initials and the pickup deadline, never the name', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(HoldShelfSlip, {
        title: 'Privacy',
        barcode: 'EX-1',
        callNumber: '800 HOL',
        holdFor: { id: '3', cardNumber: 'CARD-3', initials: 'HH' },
        pickupDeadline: '2026-10-20T18:00:00.000Z',
      }),
    );
    assert.match(html, /carte n°/);
    assert.match(html, /CARD-3/);
    assert.match(html, /Date limite de retrait/);
    assert.match(html, /exemplaire|Privacy|EX-1|800 HOL/);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('spécimen'), false);
    assert.equal(html.includes('specimen'), false);

    const fromFull = render(
      await i18nFor('en'),
      createElement(HoldShelfSlip, {
        title: 'Privacy',
        barcode: null,
        callNumber: null,
        holdFor: {
          id: '3',
          cardNumber: null,
          initials: 'HH',
        },
        pickupDeadline: null,
      }),
    );
    assert.match(fromFull, /HH/);
    assert.match(fromFull, /Pickup deadline/);
    assert.equal(fromFull.includes('Helene'), false);
    assert.equal(fromFull.includes('card number'), false);
  });
});
