import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import en from '../locales/en/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import SubscriptionFormFields from '@/components/serials/SubscriptionFormFields';
import ReceiveIssueDialog, { ReceivedSerialCopy } from '@/components/serials/ReceiveIssueDialog';
import type { SerialFrequency, SerialIssue } from '@/types';
import {
  buildSubscriptionWrite,
  centsToEurosInput,
  CREATE_ITEMS_DEFAULT,
  defaultGraceDays,
  emptySubscriptionForm,
  eurosToCents,
  monthChoices,
  nextReceivableIssueId,
  previewDateSpan,
  skipMonths,
  toggleSkipMonth,
} from '@/utils/serialSubscriptionDisplay';

const MONTHLY: SerialFrequency = {
  id: '7',
  description: '1/month',
  unit: 'month',
  issuesPerUnit: 1,
  unitsPerIssue: 1,
  displayOrder: 1,
};

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: { fr: { translation: fr }, en: { translation: en } },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nextProvider, { i18n }, node));
}

function issue(id: string, status: SerialIssue['status']): SerialIssue {
  return {
    id,
    serialSubscriptionId: '1',
    plannedDate: '2024-01-31',
    displayLabel: `n° ${id}`,
    x: 1,
    y: 0,
    z: 0,
    status,
    supplement: false,
  };
}

describe('serial subscription amounts and grace', () => {
  it('keeps an empty amount null and converts euros to cents', () => {
    assert.deepEqual(eurosToCents(''), { ok: true, cents: null });
    assert.deepEqual(eurosToCents('   '), { ok: true, cents: null });
    assert.deepEqual(eurosToCents('48'), { ok: true, cents: 4800 });
    assert.deepEqual(eurosToCents('48,50'), { ok: true, cents: 4850 });
    assert.deepEqual(eurosToCents('0'), { ok: true, cents: 0 });
    assert.equal(centsToEurosInput(null), '');
    assert.equal(centsToEurosInput(4800), '48.00');
    assert.equal(CREATE_ITEMS_DEFAULT, true);
    assert.equal(defaultGraceDays('month', 1, 1), 15);
    assert.equal(defaultGraceDays('month', 2, 1), 7);
    assert.equal(defaultGraceDays(null, 1, 1), null);
  });

  it('builds a write body with null cents and create_items on by default', () => {
    const values = {
      ...emptySubscriptionForm(),
      biblioId: '10',
      siteId: '2',
      frequencyId: '7',
      numberingPatternId: '1',
      firstIssueDate: '2024-01-31',
      costEuros: '',
      issuePriceEuros: '6,00',
    };
    const built = buildSubscriptionWrite(values);
    assert.equal(built.ok, true);
    if (!built.ok) return;
    assert.equal(built.body.costCents, null);
    assert.equal(built.body.issuePriceCents, 600);
    assert.equal(built.body.createItems, true);
    assert.equal(built.body.graceDays, null);
    assert.deepEqual(built.body.recurringSkips, []);
  });

  it('sends month-only skips and keeps a stored day until the month list changes', () => {
    const base = {
      ...emptySubscriptionForm(),
      biblioId: '10',
      siteId: '2',
      frequencyId: '7',
      numberingPatternId: '1',
      firstIssueDate: '2024-01-31',
      recurringSkips: '8, 12',
    };
    const months = buildSubscriptionWrite(base);
    assert.equal(months.ok, true);
    if (!months.ok) return;
    assert.deepEqual(months.body.recurringSkips, [{ month: 8 }, { month: 12 }]);

    const withDay = buildSubscriptionWrite({ ...base, recurringSkips: '7/15' });
    assert.equal(withDay.ok, true);
    if (!withDay.ok) return;
    assert.deepEqual(withDay.body.recurringSkips, [{ month: 7, day: 15 }]);
    assert.deepEqual(skipMonths('7/15, 8'), [7, 8]);
    assert.equal(toggleSkipMonth('7/15, 8', 1), '1, 7, 8');
    assert.equal(toggleSkipMonth('7/15, 8', 7), '8');
    assert.deepEqual(monthChoices(), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
  });
});

describe('serial subscription form', () => {
  it('shows the frequency grace as placeholder and checks create copy by default', async () => {
    const fr = await i18nFor('fr');
    const values = {
      ...emptySubscriptionForm(),
      frequencyId: MONTHLY.id,
      costEuros: centsToEurosInput(4800),
    };
    const html = render(
      fr,
      createElement(SubscriptionFormFields, {
        values,
        onChange: () => undefined,
        errors: {},
        frequencies: [MONTHLY],
        patterns: [],
        sites: [],
        vendors: [],
        funds: [],
        biblioQuery: '',
        onBiblioQuery: () => undefined,
        biblioResults: [],
        biblioSearching: false,
        onSelectBiblio: () => undefined,
        predictionLocked: false,
        preview: [],
        previewLoading: false,
        previewError: null,
        emptyHorizon: false,
      }),
    );
    assert.match(html, /id="serial-grace"/);
    assert.match(html, /placeholder="15"/);
    assert.match(html, /id="serial-create-items"/);
    assert.match(html, /checked/);
    assert.match(html, /value="48\.00"/);
    assert.match(html, /mensuel/);
    assert.match(html, /Créer un exemplaire lors du bulletinage/);
    assert.match(html, /role="combobox"/);
    assert.match(html, /id="serial-pattern"/);
    assert.match(html, /aria-expanded="false"/);
    assert.doesNotMatch(html, /id="serial-status"/);
    assert.doesNotMatch(html, /Échu|Clos/);
    assert.doesNotMatch(html, /spécimen/i);
    assert.doesNotMatch(html, /<fieldset/);
  });

  it('lists one checkbox per month inside a fieldset', async () => {
    const fr = await i18nFor('fr');
    const html = render(
      fr,
      createElement(SubscriptionFormFields, {
        values: { ...emptySubscriptionForm(), recurringSkips: '8' },
        onChange: () => undefined,
        errors: {},
        frequencies: [],
        patterns: [],
        sites: [],
        vendors: [],
        funds: [],
        biblioQuery: 'revue',
        onBiblioQuery: () => undefined,
        biblioResults: [{ id: '1', title: 'La Revue des fleuves' } as never],
        biblioSearching: false,
        onSelectBiblio: () => undefined,
        predictionLocked: false,
        preview: [],
        previewLoading: false,
        previewError: null,
        emptyHorizon: false,
        initialAdvancedOpen: true,
      }),
    );
    const fieldset = html.match(/<fieldset[\s\S]*<\/fieldset>/);
    assert.ok(fieldset);
    assert.match(fieldset[0], /Mois sans parution \(chaque année\)/);
    assert.equal((fieldset[0].match(/type="checkbox"/g) ?? []).length, 12);
    assert.match(fieldset[0], /checked/);
    assert.doesNotMatch(fieldset[0], /<select/);
    assert.match(html, /role="listbox"/);
  });

  it('shows the empty-horizon message beside the preview', async () => {
    const en = await i18nFor('en');
    const html = render(
      en,
      createElement(SubscriptionFormFields, {
        values: emptySubscriptionForm(),
        onChange: () => undefined,
        errors: {},
        frequencies: [],
        patterns: [],
        sites: [],
        vendors: [],
        funds: [],
        biblioQuery: '',
        onBiblioQuery: () => undefined,
        biblioResults: [],
        biblioSearching: false,
        onSelectBiblio: () => undefined,
        predictionLocked: false,
        preview: [],
        previewLoading: false,
        previewError: 'This horizon produced no issues',
        emptyHorizon: true,
      }),
    );
    const live = html.match(/<p aria-live="polite"[^>]*>[\s\S]*?<\/p>/);
    assert.ok(live);
    assert.doesNotMatch(live[0], /<table/);
    assert.match(html, /<table/);
    assert.match(html, /<caption/);
    assert.match(html, /role="alert"/);
    assert.match(html, /This length produced no issues/);
    const span = previewDateSpan(
      [{ plannedDate: '2027-01-05' }, { plannedDate: '2027-12-21' }],
      'fr',
    );
    assert.equal(span?.from, '5 janvier');
    assert.equal(span?.to, '21 décembre 2027');
  });
});

describe('serial issue receive flow', () => {
  it('moves to the next receivable issue and shows the created barcode', async () => {
    const issues = [issue('412', 'expected'), issue('413', 'late'), issue('414', 'arrived')];
    assert.equal(nextReceivableIssueId(issues, '412'), '413');
    assert.equal(nextReceivableIssueId(issues, '413'), '412');

    const fr = await i18nFor('fr');
    const dialog = render(
      fr,
      createElement(ReceiveIssueDialog, {
        isOpen: true,
        label: 'n° 412, mars 2026',
        createItems: true,
        isSaving: false,
        error: null,
        onClose: () => undefined,
        onSubmit: () => undefined,
      }),
    );
    assert.match(dialog, /n° 412, mars 2026/);
    assert.match(dialog, /Bulletiner le numéro/);
    assert.match(dialog, /Bulletiner n° 412, mars 2026/);
    assert.doesNotMatch(dialog, /Réceptionner/);
    assert.match(dialog, /id="serial-receive-barcode"/);
    assert.match(dialog, /aria-modal="true"/);

    const result = render(
      fr,
      createElement(ReceivedSerialCopy, {
        label: 'n° 412, mars 2026',
        barcode: 'SER-412',
        onDismiss: () => undefined,
      }),
    );
    assert.match(result, /SER-412/);
    assert.match(result, /n° 412, mars 2026/);
    assert.match(result, /aria-live="polite"/);
    assert.match(result, /Exemplaire créé/);
  });
});
