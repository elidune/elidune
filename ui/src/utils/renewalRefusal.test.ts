import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { circulationMayForce } from './circulationForce.ts';
import {
  accountRenewalRefusal,
  getApiErrorMessage,
  isAccountRenewalRefusal,
  renewalRefusalErrorMessage,
} from './apiError.ts';

function httpError(status: number, code: string, message = 'server') {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: { code, message } },
  };
}

async function tFor(lng: 'fr' | 'en' | 'de' | 'es'): Promise<TFunction> {
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

describe('account renewal refusals', () => {
  it('prefers a lost card, then a blocked account, then an expired subscription, then fines', () => {
    assert.equal(accountRenewalRefusal([]), null);
    assert.equal(accountRenewalRefusal(['hold_waiting', null]), null);
    assert.equal(accountRenewalRefusal(['fines_over_threshold']), 'fines_over_threshold');
    assert.equal(
      accountRenewalRefusal(['fines_over_threshold', 'subscription_expired']),
      'subscription_expired',
    );
    assert.equal(
      accountRenewalRefusal(['subscription_expired', 'account_blocked', 'fines_over_threshold']),
      'account_blocked',
    );
    assert.equal(
      accountRenewalRefusal(['account_blocked', 'card_lost', 'subscription_expired']),
      'card_lost',
    );
    assert.equal(isAccountRenewalRefusal('card_lost'), true);
    assert.equal(isAccountRenewalRefusal('subscription_expired'), true);
    assert.equal(isAccountRenewalRefusal('hold_waiting'), false);
  });

  it('localizes a 422 subscription_expired and leaves card_lost on the staff checkout sentence', async () => {
    const t = await tFor('fr');
    const expired = httpError(422, 'subscription_expired', 'User subscription expired on 2020-01-15 — use force=true to override');
    assert.equal(
      renewalRefusalErrorMessage(expired, t),
      'Abonnement expiré : renouvellement impossible',
    );
    assert.equal(getApiErrorMessage(expired, t), 'Abonnement expiré : renouvellement impossible');
    assert.equal(expired.response.data.message.includes('force=true'), true);

    const lost = httpError(422, 'card_lost', 'Library card is reported lost or stolen');
    assert.equal(renewalRefusalErrorMessage(lost, t), null);
    assert.equal(
      getApiErrorMessage(lost, t),
      'Carte déclarée perdue : vérifier l\'identité et remplacer la carte',
    );
  });

  it('lets the desk force an expired subscription and never a lost card', () => {
    assert.equal(circulationMayForce('renewal', 'subscription_expired'), true);
    assert.equal(circulationMayForce('renewal', 'fines_over_threshold'), true);
    assert.equal(circulationMayForce('renewal', 'account_blocked'), true);
    assert.equal(circulationMayForce('renewal', 'card_lost'), false);
    assert.equal(circulationMayForce('loan', 'card_lost'), false);
    assert.equal(circulationMayForce('loan', 'subscription_expired'), false);
  });
});
