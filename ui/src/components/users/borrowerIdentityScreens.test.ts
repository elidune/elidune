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
import OverdueBorrowerSummary from '@/components/loans/OverdueBorrowerSummary';
import ReturnBorrowerLine from '@/components/loans/ReturnBorrowerLine';
import UserAggregateStatsView from '@/components/stats/UserAggregateStatsView';
import UserRecordLoanIdentity from '@/components/users/UserRecordLoanIdentity';
import type { LoanListItem, UserAggregateStats } from '@/types';
import { isLoanDetailsStaff, isUserRecordLoan } from '@/utils/catalogView';

type Lang = 'fr' | 'en' | 'de' | 'es';

const CARD_PHRASE: Record<Lang, string> = {
  fr: 'carte n°',
  en: 'card number',
  de: 'Leseausweis-Nr.',
  es: 'n.º de carné',
};

const AGE_UNKNOWN: Record<Lang, string> = {
  fr: 'non renseigné',
  en: 'not specified',
  de: 'nicht angegeben',
  es: 'no indicado',
};

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
  return renderToStaticMarkup(tree);
}

const FULL_OVERDUE = {
  id: '7',
  firstname: 'Marie',
  lastname: 'Curie',
  email: null as string | null,
  phone: '0600000000',
};

const MINIMAL = {
  id: '7',
  cardNumber: 'L-10042',
  initials: 'MC',
};

const INITIALS_ONLY = {
  id: '7',
  cardNumber: null,
  initials: 'MC',
};

const STAFF_USER = {
  id: '7',
  firstname: 'Marie',
  lastname: 'Curie',
  login: 'mcurie',
  barcode: 'C-9',
};

function loan(user: LoanListItem['user'], title: string, staff: boolean): LoanListItem {
  return {
    id: 'loan-1',
    itemId: 'item-1',
    startDate: '2026-01-01T00:00:00Z',
    expiryAt: '2026-02-01T00:00:00Z',
    renewalDate: null,
    nbRenews: 0,
    returnedAt: null,
    biblio: {
      id: 'biblio-1',
      mediaType: 'printedText',
      isbn: null,
      title,
      date: null,
      status: 0,
      isValid: true,
      archivedAt: null,
      ...(staff ? { weedingStatus: 'active' as const } : {}),
      author: null,
      items: [],
    },
    user,
    itemIdentification: 'EX-1',
    isOverdue: false,
  } as LoanListItem;
}

describe('overdue list borrower', () => {
  it('shows name, phone, and a record link for the full shape, and omits a blank email', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(OverdueBorrowerSummary, { user: FULL_OVERDUE, count: 2, linked: true }),
    );
    assert.match(html, /Marie Curie/);
    assert.match(html, /0600000000/);
    assert.equal(html.includes('data-reader-contact="email"'), false);
    assert.match(html, /data-reader-contact="phone"/);
    assert.match(html, /href="\/users\/7"/);
    assert.equal(html.includes('carte n°'), false);
    assert.equal(html.includes('L-10042'), false);
  });

  it('shows the library card, not a name or an email column, without read permission', async () => {
    for (const lng of ['fr', 'en', 'de', 'es'] as const) {
      const html = render(
        await i18nFor(lng),
        createElement(OverdueBorrowerSummary, { user: MINIMAL, count: 1, linked: false }),
      );
      assert.equal(html.includes('Marie'), false, lng);
      assert.equal(html.includes('Curie'), false, lng);
      assert.equal(html.includes('href='), false, lng);
      assert.equal(html.includes('data-reader-contact'), false, lng);
      assert.equal(html.includes(CARD_PHRASE[lng]), true, lng);
      assert.equal(html.includes('L-10042'), true, lng);
      assert.equal(html.toLowerCase().includes('code-barres'), false, lng);
      assert.equal(html.toLowerCase().includes('barcode'), false, lng);
    }
  });

  it('shows server initials when the minimized borrower has no card', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(OverdueBorrowerSummary, { user: INITIALS_ONLY, count: 1, linked: false }),
    );
    assert.match(html, />MC</);
    assert.equal(html.includes('carte n°'), false);
    assert.equal(html.includes('href='), false);
  });
});

describe('return detail borrower', () => {
  it('shows the staff name and no email or phone', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(ReturnBorrowerLine, { user: STAFF_USER, linked: true }),
    );
    assert.match(html, /Marie Curie/);
    assert.match(html, /href="\/users\/7"/);
    assert.equal(html.includes('data-reader-contact'), false);
    assert.equal(html.includes('mcurie'), false);
    assert.equal(html.includes('carte n°'), false);
  });

  it('shows the card or initials for the minimized return borrower', async () => {
    const card = render(
      await i18nFor('de'),
      createElement(ReturnBorrowerLine, { user: MINIMAL, linked: false }),
    );
    assert.equal(card.includes('Leseausweis-Nr.'), true);
    assert.equal(card.includes('L-10042'), true);
    assert.equal(card.includes('href='), false);
    assert.equal(card.includes('Marie'), false);

    const initials = render(
      await i18nFor('es'),
      createElement(ReturnBorrowerLine, { user: INITIALS_ONLY, linked: false }),
    );
    assert.match(initials, />MC</);
    assert.equal(initials.includes('carné'), false);
    assert.equal(initials.includes('href='), false);
  });
});

describe('reader record loans', () => {
  const staffLoan = loan(STAFF_USER, 'Traité de radioactivité', true);
  const ownLoan = loan(MINIMAL, 'La radioactivité', false);

  it('keeps both the staff row and the reader’s own minimized row', () => {
    assert.equal(isLoanDetailsStaff(staffLoan), true);
    assert.equal(isLoanDetailsStaff(ownLoan), false);
    assert.equal(isUserRecordLoan(staffLoan), true);
    assert.equal(isUserRecordLoan(ownLoan), true);
  });

  it('shows the staff name in the loan detail and hides it on the table row', async () => {
    const i18n = await i18nFor('fr');
    const row = render(
      i18n,
      createElement(UserRecordLoanIdentity, { loan: staffLoan, linked: true, placement: 'row' }),
    );
    assert.equal(row, '');
    const detail = render(
      i18n,
      createElement(UserRecordLoanIdentity, { loan: staffLoan, linked: true, placement: 'detail' }),
    );
    assert.match(detail, /Lecteur/);
    assert.match(detail, /Marie Curie/);
    assert.match(detail, /href="\/users\/7"/);
    assert.equal(detail.includes('data-reader-contact'), false);
    assert.equal(detail.includes('@'), false);
  });

  it('shows the card on the reader’s own loan without a record link', async () => {
    const i18n = await i18nFor('fr');
    const row = render(
      i18n,
      createElement(UserRecordLoanIdentity, { loan: ownLoan, linked: false, placement: 'row' }),
    );
    assert.match(row, /carte n°/);
    assert.match(row, /L-10042/);
    assert.equal(row.includes('Marie'), false);
    assert.equal(row.includes('href='), false);
    const detail = render(
      i18n,
      createElement(UserRecordLoanIdentity, { loan: ownLoan, linked: false, placement: 'detail' }),
    );
    assert.match(detail, /Lecteur/);
    assert.match(detail, /carte n° L-10042/);
    assert.equal(detail.includes('href='), false);
  });
});

describe('user stats aggregates', () => {
  const stats: UserAggregateStats = {
    usersTotal: 40,
    newUsersTotal: 5,
    activeBorrowersTotal: 12,
    groupsTotal: 3,
    usersByPublicType: [{ label: 'adult', value: 30 }],
    usersByAgeBand: [
      { label: '0-17', value: 4 },
      { label: '18-29', value: 6 },
      { label: '30-49', value: 8 },
      { label: '50-64', value: 7 },
      { label: '65+', value: 9 },
      { label: 'unknown', value: 6 },
    ],
    usersByAccountType: [{ label: 'reader', value: 36 }],
    usersBySex: [
      { label: 'female', value: 22 },
      { label: 'unknown', value: 2 },
    ],
    newUsersByPublicType: [{ label: 'children', value: 2 }],
    newUsersBySex: [{ label: 'male', value: 3 }],
    activeBorrowersByPublicType: [{ label: 'adult', value: 10 }],
  };

  it('renders aggregates and translated series, with no nominative ranking', async () => {
    for (const lng of ['fr', 'en', 'de', 'es'] as const) {
      const html = render(await i18nFor(lng), createElement(UserAggregateStatsView, { stats }));
      assert.equal(html.includes('>40<') || html.includes('>40'), true, lng);
      assert.match(html, />3</);
      assert.match(html, /0-17/);
      assert.match(html, /18-29/);
      assert.match(html, /30-49/);
      assert.match(html, /50-64/);
      assert.match(html, /65\+/);
      assert.equal(html.includes(AGE_UNKNOWN[lng]), true, lng);
      assert.equal(html.includes('sortBy'), false, lng);
      assert.equal(html.includes('modeLeaderboard'), false, lng);
      assert.equal(html.includes('Marie'), false, lng);
    }
    const fr = render(await i18nFor('fr'), createElement(UserAggregateStatsView, { stats }));
    assert.match(fr, /Groupes/);
    assert.match(fr, /Lecteur/);
    assert.match(fr, /Féminin/);
    assert.match(fr, /Masculin/);
    assert.match(fr, /non spécifié/);
    assert.match(fr, /non renseigné/);
    assert.equal(fr.includes('unknown'), false);
  });
});
