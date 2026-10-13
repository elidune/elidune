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
import type { BiblioPublicView, BiblioStaffView } from '@/types';
import NoticeHoldsWaiting from './NoticeHoldsWaiting.tsx';

type Lang = 'fr' | 'en' | 'de' | 'es';

const PLURAL: Record<Lang, string> = {
  fr: 'Réservations en attente : 3',
  en: 'Holds waiting: 3',
  de: 'Wartende Vormerkungen: 3',
  es: 'Reservas en espera: 3',
};

const SINGULAR: Record<Lang, string> = {
  fr: 'Réservation en attente : 1',
  en: 'Hold waiting: 1',
  de: 'Wartende Vormerkung: 1',
  es: 'Reserva en espera: 1',
};

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

function render(i18n: I18nInstance, holdsCount?: number | null): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(NoticeHoldsWaiting, { holdsCount }),
    ) as ReactElement,
  );
}

describe('notice pending holds', () => {
  it('shows the count when holdsCount is greater than zero', async () => {
    for (const lng of Object.keys(PLURAL) as Lang[]) {
      const html = render(await i18nFor(lng), 3);
      assert.equal(html.includes('data-notice-holds="waiting"'), true);
      assert.equal(html.includes(PLURAL[lng]), true);
      assert.equal(html.includes(SINGULAR[lng]), false);
    }
    const one = render(await i18nFor('en'), 1);
    assert.equal(one.includes(SINGULAR.en), true);
    assert.equal(one.includes('Holds waiting:'), false);
  });

  it('hides the line when holdsCount is 0', async () => {
    const html = render(await i18nFor('fr'), 0);
    assert.equal(html, '');
    assert.equal(html.includes('Réservation'), false);
  });

  it('hides the line when holdsCount is absent', async () => {
    const missing = render(await i18nFor('en'), undefined);
    const empty = render(await i18nFor('en'), null);
    assert.equal(missing, '');
    assert.equal(empty, '');
  });

  it('shows the count on the public view without reader data', async () => {
    const publicNotice = {
      holdsCount: 2,
    } satisfies Pick<BiblioPublicView, 'holdsCount'>;
    const staffNotice = {
      holdsCount: 2,
      weedingStatus: 'onShelf',
    } satisfies Pick<BiblioStaffView, 'holdsCount' | 'weedingStatus'>;

    const i18n = await i18nFor('fr');
    const publicHtml = render(i18n, publicNotice.holdsCount);
    const staffHtml = render(i18n, staffNotice.holdsCount);

    assert.equal(publicHtml.includes('Réservations en attente : 2'), true);
    assert.equal(publicHtml, staffHtml);
    assert.equal(publicHtml.includes('weeding'), false);
    assert.equal(publicHtml.includes('borrower'), false);
    assert.equal('weedingStatus' in publicNotice, false);
  });
});
