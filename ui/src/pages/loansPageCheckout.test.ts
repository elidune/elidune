import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import fr from '../locales/fr/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import ToPickForceLoanDialog from '../components/loans/ToPickForceLoanDialog.tsx';
import { deskBorrowErrorText, presentCheckoutRefusal } from '../utils/deskCheckoutError.ts';
import { checkoutScanStopsLoan, deskScanAlert, type DeskCopySnapshot } from '../utils/deskScanAlert.ts';

const BARCODE = 'RC-B6K-101142';
const CARD = 'C-88421';
const INITIALS = 'GH';
const NAME = 'Grace Hopper';
const SENTENCE = fr.errors.apiCode.item_to_pick_for_other;

function axiosRefusal() {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status: 422,
      data: {
        code: 'item_to_pick_for_other',
        error: 'Business Rule Violation',
        message: 'This copy is on the pick list for another patron',
        forceable: false,
        userName: NAME,
        details: {
          holdId: '11',
          userId: '9',
          cardNumber: CARD,
          initials: INITIALS,
          firstname: 'Grace',
          lastname: 'Hopper',
        },
      },
    },
  };
}

/** Copy already on the holds shelf: staff availability is `onHold`, same as `ready`. */
function toPickSnapshot(): DeskCopySnapshot {
  return {
    availability: 'onHold',
    holdFor: { id: '9', cardNumber: CARD, initials: INITIALS, firstname: 'Grace', lastname: 'Hopper' },
  };
}

async function i18nFr(): Promise<{ i18n: I18nInstance; t: TFunction }> {
  const i18n = createInstance();
  await i18n.init({
    lng: 'fr',
    resources: { fr: { translation: fr }, en: { translation: en } },
    interpolation: { escapeValue: false },
  });
  return { i18n, t: i18n.t.bind(i18n) };
}

/**
 * Markup LoansPage shows after `handleBorrow`: the set-aside popup only when
 * `presentCheckoutRefusal` says so, otherwise the inline alert BorrowForm renders
 * from `deskBorrowErrorText`.
 */
function renderLoansCheckout(
  i18n: I18nInstance,
  view: ReturnType<typeof presentCheckoutRefusal>,
  error: unknown,
  t: TFunction,
): string {
  const nodes: ReactElement[] = [];
  if (view.kind === 'setAside') {
    nodes.push(
      createElement(ToPickForceLoanDialog, {
        isOpen: true,
        message: view.message,
        onCancel: () => undefined,
        onSetAside: () => undefined,
        onForce: view.force ? () => undefined : undefined,
      }),
    );
  }
  if (view.kind === 'sentence') {
    nodes.push(
      createElement(
        'div',
        { role: 'alert' },
        createElement('p', null, deskBorrowErrorText(error, t)),
      ),
    );
  }
  return renderToStaticMarkup(
    createElement(I18nextProvider, { i18n }, createElement('div', null, ...nodes)) as ReactElement,
  );
}

describe('LoansPage checkout of a copy already set aside', () => {
  it('shows only the sentence for item_to_pick_for_other, with no popup and no identity', async () => {
    const { i18n, t } = await i18nFr();
    const snapshot = toPickSnapshot();
    const scan = deskScanAlert(snapshot, { patronId: '1', revealIdentity: true });
    assert.equal(scan?.kind, 'onHold');
    assert.equal(checkoutScanStopsLoan(scan), false, 'onHold must reach POST /loans');

    const error = axiosRefusal();
    const view = presentCheckoutRefusal(error, { canForce: true, alreadyForced: false, t });
    assert.equal(view.kind, 'sentence');
    assert.equal('alert' in view, false);

    const html = renderLoansCheckout(i18n, view, error, t);
    assert.equal(deskBorrowErrorText(error, t), SENTENCE);
    assert.equal(html.includes('Exemplaire réservé et mis de côté'), true);
    assert.equal(html.includes('étagère des réservations'), true);
    assert.equal(html.includes(fr.loans.toPickForOtherTitle), false);
    assert.equal(html.includes(fr.loans.toPickSetAside), false);
    assert.equal(html.includes(fr.common.cancel), false);
    assert.equal(html.includes('Forcer'), false);
    assert.equal(html.includes(CARD), false);
    assert.equal(html.includes(INITIALS), false);
    assert.equal(html.includes(NAME), false);
    assert.equal(html.includes('Grace'), false);
    assert.equal(html.includes('Hopper'), false);
    assert.equal(html.includes(BARCODE), false);
    assert.equal(html.includes(en.errors.apiCode.item_to_pick_for_other), false);

    const page = readFileSync(fileURLToPath(new URL('./LoansPage.tsx', import.meta.url)), 'utf8');
    assert.equal(page.includes('presentCheckoutRefusal'), true);
    assert.equal(page.includes('readItemToPickForOther'), false);
    assert.equal(page.includes('checkoutScanStopsLoan'), true);
    assert.equal(page.includes('cardNumber={toPickAside'), false);
  });
});
