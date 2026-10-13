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
import AccountRenewalBanner from '@/components/loans/AccountRenewalBanner';
import BatchCreateItemRow from '@/components/loans/BatchCreateItemRow';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import DeskNameSearch from '@/components/loans/DeskNameSearch';
import DeskPatronStatus from '@/components/loans/DeskPatronStatus';
import MyLoanRenewActions from '@/components/loans/MyLoanRenewActions';
import { canForceLoans, type PatronBlockReason, type PatronLoanStatus } from '@/types';
import { loanRefusalMayForce, renewalMayForce } from '@/utils/circulationForce';
import { deskPatronIdentity, presentCardScanError } from '@/utils/deskPatron';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

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

function render(i18n: I18nInstance, node: ReactElement): string {
  const tree = createElement(
    I18nextProvider,
    { i18n },
    createElement(MemoryRouter, null, node),
  ) as ReactElement;
  return renderToStaticMarkup(tree).replace(/&#x27;|&apos;/g, "'");
}

function status(partial: Partial<PatronLoanStatus> = {}): PatronLoanStatus {
  return {
    subscriptionValid: true,
    subscriptionExpiresAt: '2026-12-31',
    blocked: false,
    blockReasons: [],
    activeLoans: 2,
    overdueLoans: 1,
    maxLoans: 5,
    unpaidFinesCents: 1500,
    readyHolds: 2,
    ...partial,
  };
}

describe('desk card scan identity', () => {
  it('shows the card, then initials without dots, and links only with users read', async () => {
    const i18n = await i18nFor('fr');
    const minimal = deskPatronIdentity({
      id: '7',
      cardNumber: 'L-10042',
      barcode: 'SHOULD-NOT-SHOW',
      initials: 'J.D.',
    });
    const card = render(
      i18n,
      createElement(DeskPatronStatus, { identity: minimal, linked: false, loanStatus: null }),
    );
    assert.match(card, /carte n° L-10042/);
    assert.equal(card.includes('href='), false);
    assert.equal(card.includes('SHOULD-NOT-SHOW'), false);
    assert.equal(card.includes('J.D.'), false);

    const initialsOnly = deskPatronIdentity({
      id: '7',
      cardNumber: null,
      initials: 'J.D.',
    });
    assert.equal(initialsOnly.initials, 'JD');
    const initials = render(
      i18n,
      createElement(DeskPatronStatus, { identity: initialsOnly, linked: false, loanStatus: null }),
    );
    assert.match(initials, />JD</);
    assert.equal(initials.includes('J.D.'), false);
    assert.equal(initials.includes('carte n°'), false);

    const linked = render(
      i18n,
      createElement(DeskPatronStatus, { identity: minimal, linked: true, loanStatus: null }),
    );
    assert.match(linked, /href="\/users\/7"/);

    const full = deskPatronIdentity({
      id: '7',
      firstname: 'Marie',
      lastname: 'Curie',
      barcode: 'L-10042',
    });
    const named = render(
      i18n,
      createElement(DeskPatronStatus, {
        identity: full,
        linked: true,
        loanStatus: null,
      }),
    );
    assert.match(named, /Marie Curie/);
    assert.match(named, /href="\/users\/7"/);
    assert.equal(named.includes('carte n°'), false);
    const plain = render(
      i18n,
      createElement(DeskPatronStatus, { identity: full, linked: false, loanStatus: null }),
    );
    assert.equal(plain.includes('Marie'), false);
    assert.equal(plain.includes('Curie'), false);
    assert.match(plain, /carte n° L-10042/);
    assert.equal(plain.includes('href='), false);
  });

  it('uses cardNumber, then barcode, when there is no name', () => {
    const fromBarcode = deskPatronIdentity({
      id: '7',
      cardNumber: null,
      barcode: 'BC-9',
      initials: 'JD',
    });
    assert.equal(fromBarcode.cardNumber, 'BC-9');
    assert.equal(fromBarcode.initials, 'JD');
  });
});

describe('desk loan status', () => {
  it('shows subscription, quota, overdue, fines and ready holds', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(DeskPatronStatus, {
        identity: deskPatronIdentity({ id: '7', cardNumber: 'L-1', initials: 'JD' }),
        linked: false,
        loanStatus: status(),
      }),
    );
    assert.match(html, /Abonnement valide jusqu'au/);
    assert.match(html, /2 \/ 5 prêts/);
    assert.match(html, /1 en retard/);
    assert.match(html, /Amendes impayées/);
    assert.match(html, /15,00/);
    assert.match(html, /2 réservations à retirer/);
  });

  it('translates block reasons and keeps overdue loans as a warning', async () => {
    const reasons: PatronBlockReason[] = [
      'account_blocked',
      'card_lost',
      'subscription_expired',
      'unpaid_fines',
      'max_loans',
      'overdue_loans',
    ];
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const i18n = await i18nFor(lang);
      const html = render(
        i18n,
        createElement(DeskPatronStatus, {
          identity: deskPatronIdentity({ id: '7', cardNumber: 'L-1', initials: 'JD' }),
          linked: false,
          loanStatus: status({ blocked: true, blockReasons: reasons, overdueLoans: 3 }),
        }),
      );
      const catalog = catalogs[lang].loans.blockReason;
      for (const code of reasons) {
        if (code === 'overdue_loans') continue;
        assert.equal(html.includes(catalog[code]), true, `${lang} ${code}`);
      }
      const blocked = html.slice(html.indexOf('data-patron-block'), html.indexOf('data-patron-warning'));
      assert.equal(blocked.includes(catalog.overdue_loans), false, lang);
      assert.match(html, /data-patron-warning="overdue_loans"/);
      assert.equal(html.includes(catalog.overdue_loans), true, lang);
    }
  });

  it('does not treat overdue loans alone as a block', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(DeskPatronStatus, {
        identity: deskPatronIdentity({ id: '7', cardNumber: 'L-1', initials: 'JD' }),
        linked: false,
        loanStatus: status({
          blocked: false,
          blockReasons: ['overdue_loans'],
        }),
      }),
    );
    assert.equal(html.includes('data-patron-block'), false);
    assert.equal(html.includes('Prêt impossible'), false);
    assert.match(html, /data-patron-warning="overdue_loans"/);
    assert.match(html, /prêts en retard/);
    assert.equal(html.includes('Forcer'), false);
  });
});

describe('card_lost cannot be forced', () => {
  it('never offers a force button for a lost card', async () => {
    const i18n = await i18nFor('fr');
    assert.equal(renewalMayForce('card_lost'), false);
    assert.equal(loanRefusalMayForce(undefined, true), false);
    assert.equal(loanRefusalMayForce(false, true), false);
    const html = render(
      i18n,
      createElement(CirculationForceButton, {
        forceable: false,
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.equal(html.includes('Forcer'), false);
    assert.equal(html.includes('<button'), false);
    const batch = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: {
          barcode: 'I-1',
          success: false,
          code: 'card_lost',
          error: 'Library card is reported lost or stolen',
        },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(batch, /Carte déclarée perdue : vérifier l'identité et remplacer la carte/);
    assert.equal(batch.includes('Forcer'), false);
    assert.equal(batch.includes('Library card is reported lost or stolen'), false);

    const renewal = render(
      i18n,
      createElement(CirculationForceButton, {
        forceable: renewalMayForce('card_lost'),
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.equal(renewal.includes('Forcer'), false);
  });

  it('offers force only when the refusal says forceable and the user can force', async () => {
    const i18n = await i18nFor('fr');
    assert.equal(loanRefusalMayForce(false, true), false);
    assert.equal(loanRefusalMayForce(undefined, true), false);
    assert.equal(loanRefusalMayForce(true, false), false);
    assert.equal(loanRefusalMayForce(true, true), true);
    assert.equal(renewalMayForce('card_lost'), false);
    assert.equal(renewalMayForce('hold_waiting'), false);
    assert.equal(renewalMayForce('account_blocked'), true);
    assert.equal(renewalMayForce('subscription_expired'), true);
    assert.equal(renewalMayForce('fines_over_threshold'), true);
    const token = (loansRights: string) => {
      const payload = Buffer.from(JSON.stringify({ rights: { loansRights } })).toString('base64url');
      return `e30.${payload}.sig`;
    };
    assert.equal(canForceLoans(null, token('w')), true);
    assert.equal(canForceLoans(null, token('write')), true);
    assert.equal(canForceLoans(null, token('r')), false);
    assert.equal(canForceLoans({ rights: { loansRights: 'write' } }, null), true);
    assert.equal(canForceLoans(null, null), false);
    const forced = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: { barcode: 'I-2', success: false, code: 'max_loans', error: 'quota', forceable: true },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(forced, /Ce lecteur a atteint le nombre maximum de prêts/);
    assert.match(forced, /Forcer le prêt/);
    const generic = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: { barcode: 'I-3', success: false, code: 'business_rule_violation', error: 'withdrawn' },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(generic, /règle métier/);
    assert.equal(generic.includes('Forcer'), false);
    const denied = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: {
          barcode: 'I-4',
          success: false,
          code: 'item_not_loanable',
          error: 'not borrowable',
          forceable: true,
        },
        canForce: false,
        onForce: () => undefined,
      }),
    );
    assert.match(denied, /Cet exemplaire ne peut pas être prêté/);
    assert.equal(denied.includes('Forcer'), false);
    const omitted = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: { barcode: 'I-5', success: false, code: 'max_loans', error: 'quota' },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.equal(omitted.includes('Forcer'), false);
    const held = render(
      i18n,
      createElement(BatchCreateItemRow, {
        item: {
          barcode: 'I-6',
          success: false,
          code: 'item_held_for_other',
          error: 'held',
          forceable: true,
        },
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(held, /Un autre lecteur a la priorité sur cet exemplaire/);
    assert.match(held, /Mettre de côté/);
    assert.match(held, /Annuler/);
    assert.equal(held.includes('Forcer'), false);
  });

  it('explains each item status and never offers force', async () => {
    const i18n = await i18nFor('fr');
    const expected = {
      lost: 'Utilisez « Retrouvé » avant le prêt.',
      claimedReturned: "Clôturez d'abord l'enquête.",
      toSearch: 'Marquez-le comme retrouvé avant le prêt.',
      missing: 'Marquez-le comme retrouvé avant le prêt.',
    } as const;
    for (const itemStatus of ['lost', 'claimedReturned', 'toSearch', 'missing'] as const) {
      const html = render(
        i18n,
        createElement(BatchCreateItemRow, {
          item: {
            barcode: `I-${itemStatus}`,
            success: false,
            code: 'item_status_action_required',
            error: 'Resolve it before checkout.',
            forceable: true,
            itemStatus,
          },
          canForce: true,
          onForce: () => undefined,
        }),
      );
      assert.match(html, new RegExp(expected[itemStatus].replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
      assert.match(html, /exemplaire/);
      assert.equal(html.includes('Forcer'), false, itemStatus);
      assert.equal(html.includes('Resolve it before checkout.'), false, itemStatus);
    }
  });

  it('shows an info banner and keeps the lost-card sentence', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(DeskPatronStatus, {
        identity: deskPatronIdentity({ id: '7', cardNumber: 'L-1', initials: 'JD' }),
        linked: false,
        loanStatus: status({ blocked: true, blockReasons: ['card_lost'] }),
      }),
    );
    assert.match(html, /data-card-lost-banner/);
    assert.match(html, /Carte déclarée perdue : vérifier l'identité et remplacer la carte/);
    assert.match(html, /Les retours et le paiement des amendes restent possibles/);
    assert.equal(html.includes('Forcer'), false);
  });
});

describe('desk name search', () => {
  it('is hidden without users read', async () => {
    const i18n = await i18nFor('fr');
    const hidden = render(
      i18n,
      createElement(DeskNameSearch, {
        canReadUsers: false,
        value: '',
        onChange: () => undefined,
        items: [],
        onSelect: () => undefined,
        loading: false,
        disabled: false,
        error: null,
        selectError: null,
        onRetry: () => undefined,
      }),
    );
    assert.equal(hidden.includes('Rechercher par nom'), false);
    const shown = render(
      i18n,
      createElement(DeskNameSearch, {
        canReadUsers: true,
        value: '',
        onChange: () => undefined,
        items: [],
        onSelect: () => undefined,
        loading: false,
        disabled: false,
        error: null,
        selectError: null,
        onRetry: () => undefined,
      }),
    );
    assert.match(shown, /Rechercher par nom/);
  });
});

describe('unknown card', () => {
  it('uses a neutral message and does not also toast', () => {
    const presented = presentCardScanError(404, 'forbidden duplicate', fr.loans.unknownCard);
    assert.equal(presented.message, 'Carte inconnue');
    assert.equal(presented.toast, false);
    assert.equal(presented.message.includes('forbidden'), false);
    const forbidden = presentCardScanError(403, 'Accès refusé', fr.loans.unknownCard);
    assert.equal(forbidden.message, 'Accès refusé');
    assert.equal(forbidden.toast, false);
  });
});

describe('my loans account renewal refusals', () => {
  it('keeps a lost card and an expired subscription off the loan row', async () => {
    for (const code of ['card_lost', 'subscription_expired'] as const) {
      for (const lang of ['fr', 'en', 'de', 'es'] as const) {
        const i18n = await i18nFor(lang);
        const html = render(
          i18n,
          createElement(MyLoanRenewActions, {
            canRenew: false,
            renewalRefusalCode: code,
            renewalOpensAt: null,
            onRenew: () => undefined,
          }),
        );
        assert.match(html, /disabled/, `${lang} ${code}`);
        assert.equal(html.includes(catalogs[lang].loans.renewalRefusal[code]), false, `${lang} ${code}`);
        assert.equal(html.includes('Forcer'), false, `${lang} ${code}`);
      }
    }
    const waiting = render(
      await i18nFor('fr'),
      createElement(MyLoanRenewActions, {
        canRenew: false,
        renewalRefusalCode: 'hold_waiting',
        renewalOpensAt: null,
        onRenew: () => undefined,
      }),
    );
    assert.match(waiting, /un autre lecteur attend cet exemplaire/i);
    assert.equal(waiting.includes('Forcer'), false);
    assert.equal(
      fr.loans.renewalRefusal.card_lost,
      'Carte déclarée perdue : renouvellement impossible',
    );
    assert.equal(
      fr.loans.renewalRefusal.subscription_expired,
      'Abonnement expiré, renouvelez-le à l\'accueil pour prolonger vos prêts.',
    );
  });

  it('shows one account banner for a lost card or an expired subscription', async () => {
    const expected = {
      fr: {
        card_lost: 'Carte déclarée perdue : renouvellement impossible',
        subscription_expired: 'Abonnement expiré, renouvelez-le à l\'accueil pour prolonger vos prêts.',
      },
      en: {
        card_lost: 'Card reported lost: renewal is not possible',
        subscription_expired: 'Subscription expired, renew it at the desk to extend your loans.',
      },
      de: {
        card_lost: 'Karte als verloren gemeldet: Verlängerung nicht möglich',
        subscription_expired: 'Abonnement abgelaufen, erneuern Sie es an der Ausleihe, um Ihre Ausleihen zu verlängern.',
      },
      es: {
        card_lost: 'Carné declarado perdido: renovación imposible',
        subscription_expired: 'Suscripción caducada, renuévela en el mostrador para prolongar sus préstamos.',
      },
    } as const;
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const i18n = await i18nFor(lang);
      for (const code of ['card_lost', 'subscription_expired'] as const) {
        const html = render(i18n, createElement(AccountRenewalBanner, { code }));
        assert.match(html, new RegExp(`data-account-renewal="${code}"`));
        assert.equal(html.includes(expected[lang][code]), true, `${lang} ${code}`);
        assert.equal(html.includes('Forcer'), false, `${lang} ${code}`);
      }
      const hidden = render(i18n, createElement(AccountRenewalBanner, { code: null }));
      assert.equal(hidden, '');
    }
  });

  it('offers force for an expired subscription and never for a lost card', async () => {
    const i18n = await i18nFor('fr');
    const expired = render(
      i18n,
      createElement(CirculationForceButton, {
        forceable: renewalMayForce('subscription_expired'),
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.match(expired, /Forcer/);
    const lost = render(
      i18n,
      createElement(CirculationForceButton, {
        forceable: renewalMayForce('card_lost'),
        canForce: true,
        onForce: () => undefined,
      }),
    );
    assert.equal(lost.includes('Forcer'), false);
    assert.equal(lost.includes('<button'), false);
  });
});
