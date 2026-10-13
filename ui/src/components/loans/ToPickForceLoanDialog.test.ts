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
import { readItemToPickForOther } from '@/utils/toPickDesk';

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
  props: { cardNumber?: string | null; initials?: string | null },
): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(ToPickForceLoanDialog, {
        isOpen: true,
        cardNumber: props.cardNumber ?? null,
        initials: props.initials ?? null,
        onCancel: () => undefined,
        onSetAside: () => undefined,
      }),
    ) as ReactElement,
  );
}

function refusal(details: Record<string, unknown>) {
  return {
    isAxiosError: true,
    response: {
      status: 422,
      data: {
        code: 'item_to_pick_for_other',
        message: 'This copy is on the pick list for another patron — use force=true to override',
        userName: NAME,
        details,
      },
    },
  };
}

describe('to-pick set-aside dialog', () => {
  it('shows the wrapping title, the card or initials, set aside, and cancel — never force', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const title = catalogs[lang].loans.toPickForOtherTitle;
      const setAside = catalogs[lang].loans.toPickSetAside;
      const cancel = catalogs[lang].common.cancel;
      const message = catalogs[lang].loans.toPickForOtherMessage;
      assert.equal(title.includes('{{'), false, lang);
      assert.equal(setAside.toLowerCase().includes('force'), false, lang);
      assert.equal(message.toLowerCase().includes('force'), false, lang);
      assert.equal(message.toLowerCase().includes('forc'), false, lang);
      const html = renderDialog(await i18nFor(lang), { cardNumber: CARD, initials: 'HH' });
      assert.equal(html.includes(title), true, lang);
      assert.equal(html.includes(setAside), true, lang);
      assert.equal(html.includes(cancel), true, lang);
      assert.equal(html.includes(CARD), true, lang);
      assert.equal(html.toLowerCase().includes('force'), false, lang);
      assert.equal(html.toLowerCase().includes('forc'), false, lang);
      assert.match(html, /<h2[^>]*class="[^"]*break-words[^"]*"/);
      assert.equal(/<h2[^>]*truncate/.test(html), false);
    }
    assert.equal(catalogs.fr.loans.toPickForOtherTitle, 'Exemplaire en attente pour un autre lecteur');
    assert.equal(catalogs.fr.loans.toPickSetAside, 'Mettre de côté');
    assert.equal(catalogs.fr.common.cancel, 'Annuler');
  });

  it('shows initials when the 422 has no card, and never a name', async () => {
    const parsed = readItemToPickForOther(
      refusal({
        holdId: '11',
        userId: '3',
        cardNumber: null,
        initials: 'HH',
        firstname: 'Helene',
        lastname: 'Holder',
      }),
    );
    assert.equal(parsed?.cardNumber, null);
    assert.equal(parsed?.initials, 'HH');
    assert.equal(JSON.stringify(parsed).includes(NAME), false);
    const html = renderDialog(await i18nFor('en'), {
      cardNumber: parsed?.cardNumber,
      initials: parsed?.initials,
    });
    assert.equal(html.includes('HH'), true);
    assert.equal(html.includes('Helene'), false);
    assert.equal(html.includes('Holder'), false);
    assert.equal(html.includes('Set aside'), true);
    assert.equal(html.includes('Force loan'), false);
  });

  it('reads details.cardNumber and initials and ignores a name on the 422', () => {
    const parsed = readItemToPickForOther(
      refusal({
        holdId: '11',
        userId: '3',
        cardNumber: CARD,
        initials: 'HH',
        firstname: 'Helene',
        lastname: 'Holder',
      }),
    );
    assert.deepEqual(parsed, {
      holdId: '11',
      userId: '3',
      cardNumber: CARD,
      initials: 'HH',
    });
    assert.equal(JSON.stringify(parsed).includes('Helene'), false);
    assert.equal(JSON.stringify(parsed).includes(NAME), false);
    assert.equal(
      readItemToPickForOther({ isAxiosError: true, response: { data: { code: 'card_lost' } } }),
      null,
    );
  });
});
