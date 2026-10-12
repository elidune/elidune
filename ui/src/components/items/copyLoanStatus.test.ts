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
import CopyLoanStatus from './CopyLoanStatus.tsx';
import { formatCopyDueDate, itemBorrowerIdentity } from '../../utils/copyLoanStatus.ts';

const LEAK = {
  borrowed: true,
  availability: 'onLoan',
  loanId: 'loan-99',
  dueDate: '2026-10-02T12:00:00Z',
  borrower: { id: 'user-7', cardNumber: 'L-10042', initials: 'JD' },
};

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
  });
  return i18n;
}

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(MemoryRouter, null, node),
    ) as ReactElement,
  );
}

describe('item copy loan status', () => {
  it('shows Emprunté without the loan id, and the card only when the item carries a borrower', async () => {
    const i18n = await i18nFor('fr');
    const plain = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'staff',
        copy: { borrowed: true, loanId: 'loan-99', borrower: { id: 'loan-99' } },
      }),
    );
    assert.equal(plain.includes('Emprunté'), true);
    assert.equal(plain.includes('Emprunté par'), false);
    assert.equal(plain.includes('loan-99'), false);

    const identified = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'staff',
        linked: true,
        copy: LEAK,
      }),
    );
    assert.equal(identified.includes('carte n°'), true);
    assert.equal(identified.includes('L-10042'), true);
    assert.equal(identified.includes('href="/users/user-7"'), true);
    assert.equal(identified.includes('loan-99'), false);

    const unlinked = render(
      i18n,
      createElement(CopyLoanStatus, { audience: 'staff', linked: false, copy: LEAK }),
    );
    assert.equal(unlinked.includes('L-10042'), true);
    assert.equal(unlinked.includes('href='), false);
    assert.equal(itemBorrowerIdentity({ id: '3' }), null);
  });

  it('never renders borrower identifiers on the public item view', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(CopyLoanStatus, { audience: 'public', linked: true, copy: LEAK }),
    );
    const due = formatCopyDueDate(LEAK.dueDate, 'fr');
    assert.ok(due);
    assert.equal(html.includes('data-copy-status="public"'), true);
    assert.equal(html.includes('Emprunté'), true);
    assert.equal(html.includes(`retour prévu le ${due}`), true);
    assert.equal(html.includes('L-10042'), false);
    assert.equal(html.includes('JD'), false);
    assert.equal(html.includes('user-7'), false);
    assert.equal(html.includes('loan-99'), false);
    assert.equal(html.includes('carte n°'), false);
    assert.equal(html.includes('/users/'), false);
    assert.equal(html.includes('href='), false);

    const held = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'public',
        copy: { availability: 'onHold', borrower: LEAK.borrower, loanId: 'loan-99' },
      }),
    );
    assert.equal(held.includes('Réservé'), true);
    assert.equal(held.includes('L-10042'), false);
    assert.equal(held.includes('JD'), false);
    assert.equal(held.includes('loan-99'), false);
    assert.equal(held.includes('retour prévu'), false);

    const idle = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'public',
        copy: { borrowed: false, availability: 'available', borrower: LEAK.borrower },
      }),
    );
    assert.equal(idle, '');
  });
});
