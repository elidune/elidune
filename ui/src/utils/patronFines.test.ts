import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import PatronFinesPanel from '@/components/users/PatronFinesPanel';
import type { Fine } from '@/types';
import {
  fineReasonLabel,
  fineStatusLabel,
  formatEuroAmount,
  mayLoadPatronFines,
} from './patronFines.ts';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { en, fr, de, es } as const;

const FINES: Fine[] = [
  {
    id: '1',
    userId: 'secret-user',
    loanId: 'secret-loan',
    itemId: 'secret-item',
    amount: '12.50',
    paidAmount: '0.00',
    status: 'pending',
    chargeType: 'overdue',
    notes: 'Accrual: 3 overdue days',
  },
  {
    id: '2',
    amount: '15.00',
    paidAmount: '15.00',
    refundedAmount: '15.00',
    status: 'refunded',
    chargeType: 'replacement',
  },
  {
    id: '3',
    amount: '8.00',
    paidAmount: '3.00',
    status: 'partial',
    chargeType: 'damage',
  },
  {
    id: '4',
    amount: '4.00',
    paidAmount: '0',
    status: 'cancelled',
    chargeType: 'replacement',
  },
  {
    id: '5',
    amount: '2.00',
    paidAmount: '2.00',
    status: 'waived',
  },
  {
    id: '6',
    amount: '1.00',
    paidAmount: '1.00',
    status: 'paid',
    chargeType: 'overdue',
  },
];

function keysOf(value: unknown, prefix = ''): string[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return prefix ? [prefix] : [];
  return Object.entries(value).flatMap(([key, child]) => keysOf(child, prefix ? `${prefix}.${key}` : key));
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

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nextProvider, { i18n }, node) as ReactElement);
}

describe('patron fines on the reader record', () => {
  it('loads fines only with users-read', () => {
    assert.equal(mayLoadPatronFines(true), true);
    assert.equal(mayLoadPatronFines(false), false);
  });

  it('formats decimal euro strings, including a missing refund', () => {
    assert.match(formatEuroAmount('12.50', 'fr'), /12,50/);
    assert.match(formatEuroAmount('12.50', 'fr'), /€/);
    assert.match(formatEuroAmount('12.50', 'en'), /12\.50/);
    assert.match(formatEuroAmount('12.50', 'de'), /12,50/);
    assert.match(formatEuroAmount('12.50', 'es'), /12,50/);
    assert.match(formatEuroAmount(undefined, 'fr'), /0,00/);
    assert.match(formatEuroAmount('0', 'en'), /0\.00/);
  });

  it('keeps fine copy in sync and free of emoji', () => {
    const expected = keysOf(fr.users.fines).sort();
    for (const catalog of Object.values(catalogs)) {
      assert.deepEqual(keysOf(catalog.users.fines).sort(), expected);
      const blob = JSON.stringify(catalog.users.fines);
      assert.equal(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u.test(blob), false);
    }
    assert.match(fr.users.fines.empty, /lecteur/);
    assert.match(fr.users.fines.reasons.replacement, /[Ee]xemplaire/);
    assert.equal(fr.users.finesTab, 'Amendes');
  });

  it('lists reason, amounts, and status in every locale', async () => {
    for (const lang of ['fr', 'en', 'de', 'es'] as const) {
      const i18n = await i18nFor(lang);
      const t = i18n.t.bind(i18n) as TFunction;
      const html = render(
        i18n,
        createElement(PatronFinesPanel, {
          fines: FINES,
          totalUnpaid: '12.50',
          isLoading: false,
          errorMessage: null,
          onRetry: () => undefined,
        }),
      );
      assert.equal(html.includes('secret-user'), false);
      assert.equal(html.includes('secret-loan'), false);
      assert.equal(html.includes('secret-item'), false);
      assert.match(html, /12[,.]50/);
      assert.match(html, /15[,.]00/);
      assert.match(html, /€/);
      assert.match(html, new RegExp(fineReasonLabel('replacement', t)));
      assert.match(html, new RegExp(fineReasonLabel('damage', t)));
      assert.match(html, new RegExp(fineReasonLabel('overdue', t)));
      assert.match(html, new RegExp(fineReasonLabel(undefined, t)));
      for (const status of ['pending', 'partial', 'paid', 'waived', 'cancelled', 'refunded'] as const) {
        assert.match(html, new RegExp(`data-fine-status="${status}"`));
        assert.match(html, new RegExp(fineStatusLabel(status, t)));
      }
      assert.match(html, /Accrual: 3 overdue days/);
      assert.match(html, /0[,.]00/);
    }
  });

  it('shows the server unpaid total after a cancelled or refunded settlement', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(PatronFinesPanel, {
        fines: [
          {
            id: '9',
            amount: '15.00',
            paidAmount: '0.00',
            status: 'cancelled',
            chargeType: 'replacement',
          },
        ],
        totalUnpaid: '0.00',
        isLoading: false,
        errorMessage: null,
        onRetry: () => undefined,
      }),
    );
    assert.match(html, /0,00/);
    assert.match(html, /15,00/);
    assert.match(html, /Annulée/);
    assert.match(html, /Exemplaire perdu/);
    assert.match(html, /Total impayé/);
  });

  it('shows an empty list and a load error without identity fields', async () => {
    const i18n = await i18nFor('en');
    const empty = render(
      i18n,
      createElement(PatronFinesPanel, {
        fines: [],
        totalUnpaid: '0',
        isLoading: false,
        errorMessage: null,
        onRetry: () => undefined,
      }),
    );
    assert.match(empty, /No fines for this reader/);
    assert.match(empty, /Total unpaid/);

    const failed = render(
      i18n,
      createElement(PatronFinesPanel, {
        fines: [],
        isLoading: false,
        errorMessage: 'Could not load fines.',
        onRetry: () => undefined,
      }),
    );
    assert.match(failed, /Could not load fines/);
    assert.equal(failed.includes('data-fines-total'), false);
  });
});
