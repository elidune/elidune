import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { CheckoutScanNoticeShown } from './deskScanAlert.ts';
import { circulationMayForce } from './circulationForce.ts';
import {
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

function axiosError(status: number, code: string | null, message: string) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: {
      status,
      data: code ? { code, error: 'Unprocessable Entity', message } : { message },
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

  it('maps item_to_pick_for_other with lecteur and exemplaire in French', async () => {
    const t = await tFor('fr');
    const message = deskCheckoutMessage(
      axiosError(422, 'item_to_pick_for_other', 'This copy is on the pick list for another patron — use force=true to override'),
      t,
    );
    assert.equal(message, 'Cet exemplaire est réservé pour un autre lecteur.');
    assert.equal(message.includes('usager'), false);
    assert.equal(message.includes('force'), false);
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
        assert.equal(circulationMayForce('loan', code), true);
        assert.equal(circulationMayForce('batch', code), true);
        assert.equal(checkoutMayForce(axiosError(422, code, 'server'), false), false);
      }
    }
    const fr = await tFor('fr');
    const blocked = deskCheckoutMessage(
      axiosError(422, 'business_rule_violation', 'Item is withdrawn and cannot be checked out'),
      fr,
    );
    assert.equal(blocked, catalogs.fr.errors.apiCode.business_rule_violation);
    assert.equal(checkoutMayForce(axiosError(422, 'business_rule_violation', 'in transit')), false);
    assert.equal(circulationMayForce('loan', 'business_rule_violation', true), false);
    const expiredCheckout = deskCheckoutMessage(
      axiosError(422, 'subscription_expired', 'User subscription expired on 2020-01-15 — use force=true to override'),
      fr,
    );
    assert.equal(expiredCheckout, "L'abonnement de ce lecteur est expiré.");
    assert.equal(expiredCheckout.includes('prolonger'), false);
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
