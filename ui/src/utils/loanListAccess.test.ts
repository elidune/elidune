import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import LoansAccessNotice from '@/components/loans/LoansAccessNotice';
import {
  checkoutRecap,
  deskMayLoadPatronLoans,
  isLoansForbidden,
  loansQueryRetry,
  readPatronLoans,
  userRecordQueryRetry,
} from './loanListAccess.ts';

function httpError(status: number, code?: string) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: code ? { code, message: 'no' } : { message: 'no' } },
  };
}

describe('patron loan list access', () => {
  it('treats a permission 403 as forbidden and a dead session as not', () => {
    assert.equal(isLoansForbidden(httpError(403, 'forbidden')), true);
    assert.equal(isLoansForbidden(httpError(403)), true);
    assert.equal(isLoansForbidden(httpError(403, 'authentication_failed')), false);
    assert.equal(isLoansForbidden(httpError(500)), false);
    assert.equal(loansQueryRetry(0, httpError(403)), false);
    assert.equal(userRecordQueryRetry(0, httpError(403)), false);
    assert.equal(userRecordQueryRetry(0, httpError(500)), true);
    assert.equal(loansQueryRetry(0, httpError(500)), true);
    assert.equal(loansQueryRetry(1, httpError(500)), false);
  });

  it('reads a page without throwing when the list is denied', async () => {
    const denied = await readPatronLoans(async () => {
      throw httpError(403, 'forbidden');
    });
    assert.equal(denied.ok, false);
    if (denied.ok) return;
    assert.equal(denied.denied, true);

    const loaded = await readPatronLoans(async () => ({ items: [], total: 0 }));
    assert.equal(loaded.ok, true);
  });

  it('does not call patron loans from the desk without users read', () => {
    assert.equal(deskMayLoadPatronLoans(false), false);
    assert.equal(deskMayLoadPatronLoans(true), true);
  });

  it('builds the checkout recap from the created loan when the list is missing', () => {
    const recap = checkoutRecap({
      created: { id: 'loan-1', expiryAt: '2026-11-01T00:00:00Z' },
      barcode: 'I-9',
      patronName: 'Ada Lovelace',
      noTitle: 'No title',
      listed: null,
    });
    assert.deepEqual(recap, {
      title: 'No title',
      barcode: 'I-9',
      dueDate: '2026-11-01T00:00:00Z',
      patronName: 'Ada Lovelace',
    });

    const matched = checkoutRecap({
      created: { id: 'loan-1', expiryAt: '2026-11-01T00:00:00Z' },
      barcode: 'I-9',
      patronName: 'Ada Lovelace',
      noTitle: 'No title',
      listed: {
        id: 'loan-1',
        biblio: { title: 'Principia' },
        itemIdentification: 'I-9-COPY',
        expiryAt: '2026-12-01T00:00:00Z',
      },
    });
    assert.equal(matched.title, 'Principia');
    assert.equal(matched.barcode, 'I-9-COPY');
    assert.equal(matched.dueDate, '2026-12-01T00:00:00Z');
  });
});

describe('LoansAccessNotice', () => {
  it('shows the not-allowed sentence once and no retry for a 403', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'en',
      resources: {
        en: { translation: en },
        fr: { translation: fr },
        de: { translation: de },
        es: { translation: es },
      },
    });
    const html = renderToStaticMarkup(
      createElement(
        I18nextProvider,
        { i18n },
        createElement(LoansAccessNotice, {
          error: httpError(403, 'forbidden'),
          onRetry: () => undefined,
          fallbackMessage: 'Could not load loans',
        }),
      ) as ReactElement,
    );
    const phrase = 'You are not allowed to view these loans.';
    assert.equal(html.split(phrase).length - 1, 1);
    assert.equal(html.includes('role="status"'), true);
    assert.equal(html.includes('role="alert"'), false);
    assert.equal(html.includes('Could not load loans'), false);
    assert.equal(html.includes('<button'), false);
  });

  it('uses the reader-record sentence once and still has no retry', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'fr',
      resources: { fr: { translation: fr }, en: { translation: en } },
    });
    const html = renderToStaticMarkup(
      createElement(
        I18nextProvider,
        { i18n },
        createElement(LoansAccessNotice, {
          denied: true,
          message: i18n.t('users.recordNotAllowed'),
          onRetry: () => undefined,
          fallbackMessage: 'Impossible de charger cet usager',
        }),
      ) as ReactElement,
    );
    const phrase = 'pas autorisé à consulter cette fiche lecteur.';
    assert.equal(html.split(phrase).length - 1, 1);
    assert.equal(html.includes('role="status"'), true);
    assert.equal(html.includes('<button'), false);
    assert.equal(html.includes('Réessayer'), false);
  });

  it('shows a single retry banner for a server error', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'en',
      resources: { en: { translation: en } },
    });
    const html = renderToStaticMarkup(
      createElement(
        I18nextProvider,
        { i18n },
        createElement(LoansAccessNotice, {
          error: httpError(500),
          onRetry: () => undefined,
          fallbackMessage: 'Could not load loans',
        }),
      ) as ReactElement,
    );
    assert.equal(html.split('role="alert"').length - 1, 1);
    assert.equal(html.includes('<button'), true);
    assert.equal(html.includes('You are not allowed to view these loans.'), false);
  });
});
