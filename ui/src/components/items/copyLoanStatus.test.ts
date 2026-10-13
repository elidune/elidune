import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import type { ItemPublicView, ItemStaffView } from '@/types';
import CopyLoanStatus from './CopyLoanStatus.tsx';
import PublicCopyReturnLine from './PublicCopyReturnLine.tsx';
import {
  formatCopyDueDate,
  formatLocalCalendarDate,
  itemBorrowerIdentity,
  publicExpectedReturnLine,
  publicOnLoanLabel,
} from '../../utils/copyLoanStatus.ts';

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

void (true satisfies 'dueDate' extends keyof ItemPublicView ? false : true);
void (true satisfies 'dueDate' extends keyof ItemStaffView ? true : false);

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
    assert.equal(html.includes('En prêt'), true);
    assert.equal(html.includes('Emprunté'), false);
    assert.equal(html.includes('retour prévu'), false);
    assert.equal(html.includes(due), false);
    assert.equal(html.includes(LEAK.dueDate), false);
    assert.equal(html.includes('retard'), false);
    assert.equal(html.includes('overdue'), false);
    assert.equal(html.includes('L-10042'), false);
    assert.equal(html.includes('JD'), false);
    assert.equal(html.includes('user-7'), false);
    assert.equal(html.includes('loan-99'), false);
    assert.equal(html.includes('carte n°'), false);
    assert.equal(html.includes('/users/'), false);
    assert.equal(html.includes('href='), false);

    const heldCopy = { availability: 'onHold' as const, borrower: LEAK.borrower, loanId: 'loan-99' };
    const held = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'public',
        copy: heldCopy,
      }),
    );
    assert.equal(held.includes('Réservé'), true);
    assert.equal(held.includes('L-10042'), false);
    assert.equal(held.includes('JD'), false);
    assert.equal(held.includes('loan-99'), false);
    assert.equal(held.includes('retour prévu'), false);

    const idleCopy = { borrowed: false, availability: 'available' as const, borrower: LEAK.borrower };
    const idle = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'public',
        copy: idleCopy,
      }),
    );
    assert.equal(idle, '');
  });

  it('formats a YYYY-MM-DD value as a local calendar date with no time', () => {
    assert.equal(formatLocalCalendarDate('2026-10-20', 'fr'), '20 octobre 2026');
    assert.equal(formatLocalCalendarDate('2026-10-20', 'en'), 'October 20, 2026');
    assert.equal(formatLocalCalendarDate('2026-10-20T00:00:00Z', 'en'), null);
    assert.equal(formatLocalCalendarDate('2026-02-31', 'en'), null);
    assert.equal(formatLocalCalendarDate(undefined, 'en'), null);

    const uiRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
    const result = spawnSync(
      process.execPath,
      [
        '--import',
        'tsx',
        '--input-type=module',
        '-e',
        "import { formatLocalCalendarDate } from './src/utils/copyLoanStatus.ts'; const text = formatLocalCalendarDate('2026-10-20', 'en'); if (text !== 'October 20, 2026') { console.error(text); process.exit(1); }",
      ],
      {
        cwd: uiRoot,
        env: { ...process.env, TZ: 'America/Los_Angeles' },
        encoding: 'utf8',
      },
    );
    assert.equal(result.status, 0, `${result.stderr ?? ''}\n${result.stdout ?? ''}`);
  });

  it('shows the expected return on the public copy list and never a borrower', async () => {
    const fr = await i18nFor('fr');
    const copy = {
      ...LEAK,
      expectedReturnDate: '2026-10-20',
      borrower: {
        ...LEAK.borrower,
        firstname: 'Julien',
        lastname: 'Dubois',
        email: 'j@example.com',
        phone: '0600000000',
        login: 'jdubois',
      },
    };
    assert.equal(
      publicExpectedReturnLine(fr.t, copy, 'fr'),
      'En prêt, retour prévu le 20 octobre 2026',
    );
    const html = render(
      fr,
      createElement(CopyLoanStatus, { audience: 'public', linked: true, copy }),
    );
    assert.equal(html.includes('En prêt, retour prévu le 20 octobre 2026'), true);
    assert.equal(html.includes('Emprunté'), false);
    for (const secret of [
      'L-10042',
      'JD',
      'Julien',
      'Dubois',
      'j@example.com',
      '0600000000',
      'jdubois',
      'user-7',
      'loan-99',
      'carte n°',
      '/users/',
    ]) {
      assert.equal(html.includes(secret), false, secret);
    }

    const en = await i18nFor('en');
    const opac = render(
      en,
      createElement(PublicCopyReturnLine, {
        availability: 'onLoan',
        expectedReturnDate: '2026-10-20',
      }),
    );
    assert.equal(opac.includes('On loan, due back October 20, 2026'), true);
    assert.equal(publicExpectedReturnLine(en.t, { availability: 'onLoan' }, 'en'), null);
    assert.equal(publicOnLoanLabel(en.t, { availability: 'onLoan' }, 'en'), 'On loan');
    assert.equal(publicOnLoanLabel(fr.t, { availability: 'onLoan' }, 'fr'), 'En prêt');
    const overdue = render(
      en,
      createElement(PublicCopyReturnLine, {
        availability: 'onLoan',
        expectedReturnDate: undefined,
      }),
    );
    assert.equal(overdue.includes('On loan'), true);
    assert.equal(overdue.includes('due back'), false);
    assert.equal(overdue.includes('October'), false);
    const overdueFr = render(
      fr,
      createElement(CopyLoanStatus, {
        audience: 'public',
        copy: { availability: 'onLoan', borrowed: true },
      }),
    );
    assert.equal(overdueFr.includes('En prêt'), true);
    assert.equal(overdueFr.includes('retour prévu'), false);
    assert.equal(overdueFr.includes('2020'), false);
    assert.equal(
      publicExpectedReturnLine(
        en.t,
        { availability: 'available', expectedReturnDate: '2026-10-20' },
        'en',
      ),
      null,
    );
  });

  it('keeps the staff copy status unchanged when expectedReturnDate is present', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(CopyLoanStatus, {
        audience: 'staff',
        linked: false,
        copy: { ...LEAK, expectedReturnDate: '2026-10-20' },
      }),
    );
    assert.equal(html.includes('data-copy-status="staff"'), true);
    assert.equal(html.includes('L-10042'), true);
    assert.equal(html.includes('20 octobre 2026'), false);
    assert.equal(html.includes('En prêt, retour prévu'), false);
  });
});
