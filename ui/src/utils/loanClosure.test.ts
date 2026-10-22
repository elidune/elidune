import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import ClosedLoanHistory from '@/components/loans/ClosedLoanHistory';
import type { MyLoan } from '@/types';
import { closureReasonText, myLoansRequestParams } from './loanClosure.ts';
import { formatLocaleDate } from './localeDate.ts';

const catalogs = { en, fr, de, es } as const;
type Lang = keyof typeof catalogs;
const FOUND = '2026-10-13T12:00:00.000Z';

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

function loan(partial: Partial<MyLoan> & Pick<MyLoan, 'id'>): MyLoan {
  return {
    barcode: 'EX-1',
    title: 'Notice',
    mediaType: 'printedText',
    startDate: '2026-09-01T12:00:00.000Z',
    dueDate: '2026-09-15T12:00:00.000Z',
    renewalCount: 0,
    canRenew: false,
    renewalRefusalCode: null,
    renewalOpensAt: null,
    isOverdue: false,
    ...partial,
  };
}

describe('closed loan closure reason', () => {
  it('asks only for the caller’s closed loans', () => {
    const open = myLoansRequestParams({ page: 1, perPage: 20 });
    assert.equal('archived' in open, false);
    assert.equal('userId' in open, false);
    assert.deepEqual(myLoansRequestParams({ archived: true, page: 2, perPage: 20 }), {
      page: 2,
      perPage: 20,
      archived: true,
    });
    const page = readFileSync(new URL('../pages/MyLoansPage.tsx', import.meta.url), 'utf8');
    assert.match(page, /archived:\s*true/);
    assert.match(page, /useMyLoansQuery/);
    assert.equal(page.includes('getUserLoans'), false);
  });

  it('keeps closure copy in sync, in French as Rendu / Perdu', () => {
    const expected = keysOf(fr.loans.closure).sort();
    for (const catalog of Object.values(catalogs)) {
      assert.deepEqual(keysOf(catalog.loans.closure).sort(), expected);
      assert.equal(/[\u{1F300}-\u{1FAFF}\u{2600}-\u{27BF}]/u.test(JSON.stringify(catalog.loans.closure)), false);
    }
    assert.equal(fr.loans.closure.returned, 'Rendu');
    assert.equal(fr.loans.closure.lost, 'Perdu');
    assert.equal(fr.loans.closure.lostFound, 'Perdu, retrouvé le {{date}}');
    assert.equal(fr.loans.closure.returned.toLowerCase().includes('specimen'), false);
    assert.equal(fr.loans.closure.lostFound.toLowerCase().includes('usager'), false);
  });

  it('labels returned, lost, and lost-then-found in every locale', async () => {
    for (const lang of Object.keys(catalogs) as Lang[]) {
      const i18n = await i18nFor(lang);
      const t = i18n.t.bind(i18n) as TFunction;
      assert.equal(closureReasonText({ closureReason: 'returned', foundAt: null }, t, lang), catalogs[lang].loans.closure.returned);
      assert.equal(closureReasonText({ closureReason: 'lost', foundAt: null }, t, lang), catalogs[lang].loans.closure.lost);
      assert.equal(closureReasonText({}, t, lang), null);
      const found = closureReasonText({ closureReason: 'lost', foundAt: FOUND }, t, lang);
      assert.match(found ?? '', new RegExp(formatLocaleDate(FOUND, lang).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
      assert.match(found ?? '', new RegExp(catalogs[lang].loans.closure.lost));
    }
    const frI18n = await i18nFor('fr');
    const frT = frI18n.t.bind(frI18n) as TFunction;
    assert.equal(
      closureReasonText({ closureReason: 'lost', foundAt: FOUND }, frT, 'fr'),
      `Perdu, retrouvé le ${formatLocaleDate(FOUND, 'fr')}`,
    );
  });

  it('lists only the passed closed loans and their reason, never a renew control', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(ClosedLoanHistory, {
        loans: [
          loan({ id: '1', closureReason: 'returned', foundAt: null, title: 'Rendu book' }),
          loan({
            id: '2',
            closureReason: 'lost',
            foundAt: FOUND,
            title: 'Lost copy',
            barcode: 'EX-9',
          }),
          loan({ id: '3', closureReason: 'lost', foundAt: null, title: 'Still lost' }),
        ],
      }),
    );
    assert.match(html, /Rendu/);
    assert.match(html, /Perdu, retrouvé le /);
    assert.match(html, />Perdu</);
    assert.equal(html.includes('Prolonger'), false);
    assert.equal(html.includes('secret-user'), false);
    assert.equal(html.includes('getUserLoans'), false);
  });
});
