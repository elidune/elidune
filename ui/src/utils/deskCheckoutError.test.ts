import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { CheckoutScanNoticeShown } from './deskScanAlert.ts';
import { loanRefusalMayForce } from './circulationForce.ts';
import {
  checkoutDeskActions,
  checkoutDeskActionsFromError,
  deskBorrowErrorText,
  deskCheckoutMessage,
  forceBorrowDialogMessage,
} from './deskCheckoutError.ts';
import { checkoutMayForce } from './loanConflict.ts';
import { CheckoutDeclined } from './toPickDesk.ts';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

const HOLD_RAW =
  'This copy has an active hold for another patron — only the queued patron may borrow it, or use force=true to override';

function axiosError(
  status: number,
  code: string | null,
  message: string,
  extra: Record<string, unknown> = {},
) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status,
      data: code ? { code, error: 'Unprocessable Entity', message, ...extra } : { message },
    },
  };
}

async function tFor(lng: Lang): Promise<TFunction> {
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
  return i18n.t.bind(i18n);
}

describe('desk checkout error mapping', () => {
  it('uses the API code in every locale and never the English server sentence', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const t = await tFor(lang);
      const error = axiosError(422, 'business_rule_violation', HOLD_RAW);
      const message = deskCheckoutMessage(error, t);
      const dialog = forceBorrowDialogMessage(error, t);
      const expected = catalogs[lang].errors.apiCode.business_rule_violation;

      assert.equal(message, expected);
      assert.equal(dialog.includes(expected), true);
      assert.equal(dialog.includes(catalogs[lang].loans.forceBorrowConfirm), true);
      assert.equal(message.includes('force=true'), false);
      assert.equal(dialog.includes('force=true'), false);
      assert.equal(message.includes('another patron'), false);
      assert.equal(dialog.includes(HOLD_RAW), false);
      assert.equal(message.includes('patron'), false);
      assert.equal(message.includes('specimen'), false);
      assert.equal(message.includes('usager'), false);
    }
  });

  it('maps item_to_pick_for_other as already set aside, with no identity and no actions', async () => {
    const identity = {
      forceable: true,
      userName: 'Grace Hopper',
      cardNumber: 'CARD-9',
      initials: 'GH',
      firstname: 'Grace',
      lastname: 'Hopper',
      details: {
        cardNumber: 'CARD-9',
        initials: 'GH',
        firstname: 'Grace',
        lastname: 'Hopper',
      },
    };
    const error = axiosError(
      422,
      'item_to_pick_for_other',
      'This copy is on the pick list for another patron — use force=true to override',
      identity,
    );
    const frMessage = deskCheckoutMessage(error, await tFor('fr'));
    const enMessage = deskCheckoutMessage(error, await tFor('en'));
    assert.equal(
      frMessage,
      "Exemplaire réservé et mis de côté pour un autre lecteur : replacez-le sur l'étagère des réservations.",
    );
    assert.equal(
      enMessage,
      'Copy reserved and set aside for another reader: put it back on the holds shelf.',
    );
    for (const message of [frMessage, enMessage]) {
      assert.equal(message.includes('Grace'), false);
      assert.equal(message.includes('Hopper'), false);
      assert.equal(message.includes('CARD-9'), false);
      assert.equal(message.includes('GH'), false);
      assert.equal(message.toLowerCase().includes('force'), false);
      assert.equal(message.toLowerCase().includes('usager'), false);
      assert.equal(message.toLowerCase().includes('patron'), false);
    }
    assert.equal(frMessage.includes('lecteur'), true);
    assert.equal(frMessage.includes('exemplaire') || frMessage.includes('Exemplaire'), true);
    assert.deepEqual(checkoutDeskActionsFromError(error, true), { force: false, setAside: false });
    assert.deepEqual(checkoutDeskActions('item_to_pick_for_other', true, true), { force: false, setAside: false });
    assert.equal(checkoutMayForce(error, true), true);

    const held = axiosError(422, 'item_held_for_other', 'held', { forceable: true, userName: 'Grace Hopper' });
    assert.equal(
      deskCheckoutMessage(held, await tFor('fr')),
      'Un autre lecteur a la priorité sur cet exemplaire.',
    );
    assert.equal(deskCheckoutMessage(held, await tFor('fr')).includes('Grace'), false);
    assert.deepEqual(checkoutDeskActionsFromError(held, true), { force: true, setAside: true });
    assert.deepEqual(checkoutDeskActions('item_held_for_other', true, true), { force: true, setAside: true });
    assert.deepEqual(checkoutDeskActions('item_held_for_other', true, false), { force: false, setAside: true });
    assert.deepEqual(checkoutDeskActions('item_held_for_other', false, true), { force: false, setAside: true });
    assert.deepEqual(checkoutDeskActions('item_held_for_other', undefined, true), { force: false, setAside: true });

    const quota = axiosError(422, 'max_loans', 'quota', { forceable: true });
    assert.deepEqual(checkoutDeskActionsFromError(quota, true), { force: true, setAside: false });
    assert.deepEqual(checkoutDeskActionsFromError(quota, false), { force: false, setAside: false });
    assert.deepEqual(checkoutDeskActions('max_loans', undefined, true), { force: false, setAside: false });
    assert.deepEqual(checkoutDeskActions('item_status_action_required', true, true), { force: false, setAside: false });
  });

  it('falls back to the generic error when the code has no translation', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const t = await tFor(lang);
      const message = deskCheckoutMessage(
        axiosError(422, 'not_a_real_code', 'Maximum loans reached: total (5/5) — use force=true to override'),
        t,
      );
      assert.equal(message, catalogs[lang].errors.generic);
      assert.equal(message.includes('force=true'), false);
      assert.equal(message.includes('Maximum loans'), false);
    }
  });

  it('maps each forceable checkout code and leaves business_rule_violation without force', async () => {
    const expected = {
      fr: {
        subscription_expired: "L'abonnement de ce lecteur est expiré.",
        fines_over_threshold: 'Les amendes impayées de ce lecteur dépassent le seuil autorisé.',
        max_loans: 'Ce lecteur a atteint le nombre maximum de prêts.',
        max_media_loans: 'Ce lecteur a atteint le nombre maximum de prêts pour ce type de document.',
        item_not_loanable: 'Cet exemplaire ne peut pas être prêté.',
        item_held_for_other: 'Un autre lecteur a la priorité sur cet exemplaire.',
      },
      en: {
        subscription_expired: "This reader's subscription has expired.",
        fines_over_threshold: "This reader's unpaid fines are over the allowed threshold.",
        max_loans: 'This reader has reached the maximum number of loans.',
        max_media_loans: 'This reader has reached the maximum number of loans for this document type.',
        item_not_loanable: 'This copy cannot be loaned.',
        item_held_for_other: 'Another reader has priority for this copy.',
      },
    } as const;
    for (const lang of ['fr', 'en'] as const) {
      const t = await tFor(lang);
      for (const code of Object.keys(expected[lang]) as (keyof (typeof expected)['fr'])[]) {
        const message = deskCheckoutMessage(
          axiosError(422, code, 'use force=true to override'),
          t,
        );
        assert.equal(message, expected[lang][code]);
        assert.equal(message.toLowerCase().includes('force'), false);
        assert.equal(message.toLowerCase().includes('spécimen'), false);
        assert.equal(message.toLowerCase().includes('specimen'), false);
        assert.equal(message.toLowerCase().includes('usager'), false);
        assert.equal(message.toLowerCase().includes('patron'), false);
        const flagged = axiosError(422, code, 'server', { forceable: code !== 'item_held_for_other' });
        assert.equal(checkoutMayForce(flagged), code !== 'item_held_for_other');
        assert.equal(checkoutMayForce(axiosError(422, code, 'server', { forceable: true }), false), false);
        assert.equal(checkoutMayForce(axiosError(422, code, 'server')), false);
        assert.equal(loanRefusalMayForce(undefined, true), false);
      }
    }
    const fr = await tFor('fr');
    const blocked = deskCheckoutMessage(
      axiosError(422, 'business_rule_violation', 'Item is withdrawn and cannot be checked out'),
      fr,
    );
    assert.equal(blocked, catalogs.fr.errors.apiCode.business_rule_violation);
    assert.equal(checkoutMayForce(axiosError(422, 'business_rule_violation', 'in transit', { forceable: false })), false);
    assert.equal(
      checkoutMayForce(axiosError(422, 'subscription_expired', 'expired', { forceable: false }), true),
      false,
    );
    assert.equal(
      checkoutMayForce(axiosError(422, 'subscription_expired', 'expired', { forceable: true }), true),
      true,
    );
    const expiredCheckout = deskCheckoutMessage(
      axiosError(422, 'subscription_expired', 'User subscription expired on 2020-01-15 — use force=true to override'),
      fr,
    );
    assert.equal(expiredCheckout, "L'abonnement de ce lecteur est expiré.");
    assert.equal(expiredCheckout.includes('prolonger'), false);
  });

  it('tells the agent what to do for each item status and never treats it as forceable', async () => {
    const expected = {
      fr: {
        lost: 'Cet exemplaire est perdu. Utilisez « Retrouvé » avant le prêt.',
        claimedReturned: "Cet exemplaire est dit rendu. Clôturez d'abord l'enquête.",
        toSearch: 'Cet exemplaire est à rechercher. Marquez-le comme retrouvé avant le prêt.',
        missing: 'Cet exemplaire est manquant. Marquez-le comme retrouvé avant le prêt.',
      },
      en: {
        lost: 'This copy is lost. Use “Found” before the loan.',
        claimedReturned: 'This copy is claimed returned. Close the investigation first.',
        toSearch: 'This copy is to search. Mark it as found before the loan.',
        missing: 'This copy is missing. Mark it as found before the loan.',
      },
    } as const;
    for (const lang of ['fr', 'en'] as const) {
      const t = await tFor(lang);
      for (const itemStatus of ['lost', 'claimedReturned', 'toSearch', 'missing'] as const) {
        const error = axiosError(422, 'item_status_action_required', 'Item circulation status is lost.', {
          forceable: false,
          itemStatus,
        });
        const message = deskCheckoutMessage(error, t);
        assert.equal(message, expected[lang][itemStatus]);
        assert.equal(message.toLowerCase().includes('force'), false);
        assert.equal(message.toLowerCase().includes('specimen'), false);
        assert.equal(message.toLowerCase().includes('usager'), false);
        assert.equal(checkoutMayForce(error), false);
      }
      const unknown = deskCheckoutMessage(
        axiosError(422, 'item_status_action_required', 'Item circulation status is damaged.', {
          forceable: false,
          itemStatus: 'damaged',
        }),
        t,
      );
      assert.equal(unknown, catalogs[lang].errors.itemStatusAction.generic);
      const absent = deskCheckoutMessage(
        axiosError(422, 'item_status_action_required', 'Resolve it before checkout.', { forceable: false }),
        t,
      );
      assert.equal(absent, catalogs[lang].errors.itemStatusAction.generic);
    }
    for (const lang of ['de', 'es'] as const) {
      const t = await tFor(lang);
      for (const itemStatus of ['lost', 'claimedReturned', 'toSearch', 'missing'] as const) {
        const message = deskCheckoutMessage(
          axiosError(422, 'item_status_action_required', 'status', { itemStatus, forceable: false }),
          t,
        );
        assert.equal(message, catalogs[lang].errors.itemStatusAction[itemStatus]);
        assert.equal(message.includes('{{'), false);
        assert.equal(message.toLowerCase().includes('force'), false);
      }
    }
  });

  it('stays silent when the librarian cancels or a scan notice already explains the refusal', async () => {
    const t = await tFor('en');
    assert.equal(deskBorrowErrorText(new CheckoutDeclined(), t), null);
    assert.equal(deskBorrowErrorText(new CheckoutScanNoticeShown(), t), null);
    assert.equal(
      deskBorrowErrorText(axiosError(422, 'card_lost', 'Library card is reported lost or stolen'), t),
      catalogs.en.errors.apiCode.card_lost,
    );
  });
});
