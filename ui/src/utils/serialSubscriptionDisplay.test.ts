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
  nextReceivableIssueId,
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
    assert.match(html, /Créer un exemplaire à la réception/);
    assert.doesNotMatch(html, /spécimen/i);
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
    assert.match(html, /aria-live="polite"/);
    assert.match(html, /<caption/);
    assert.match(html, /role="alert"/);
    assert.match(html, /This horizon produced no issues/);
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
