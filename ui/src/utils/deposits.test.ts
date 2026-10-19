import './testLocalStorage.ts';
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance, type TFunction } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import api from '../services/api.ts';
import type { ItemDeposit } from '../types/index.ts';
import { checkoutDeskActions } from './deskCheckoutError.ts';
import { checkoutMayForce } from './loanConflict.ts';
import { presentRenewalRefusal } from './renewalDesk.ts';
import { getApiErrorMessage } from './apiError.ts';
import {
  biblioMatchesDepositFilter,
  buildMarcBatchImportParams,
  depositBadgeText,
  depositDueDateNotice,
  depositScanAlreadyReturned,
  lenderNameConflictMessage,
  marcImportFailureMessage,
  parseDepositCloseReport,
  withDepositDueNotice,
} from './deposits.ts';

const deposit: ItemDeposit = {
  batchId: '10',
  lenderId: '3',
  lenderName: 'Médiathèque départementale',
  reference: 'LOT-2026',
  returnDueOn: '2026-09-01',
  batchStatus: 'active',
};

async function tFor(lng: 'fr' | 'en'): Promise<TFunction> {
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

function axiosError(status: number, code: string, message = 'server', extra: Record<string, unknown> = {}) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: { code, error: 'error', message, ...extra } },
  };
}

describe('deposit badge', () => {
  it('formats the sober French badge and hides own stock', async () => {
    const t = await tFor('fr');
    assert.equal(
      depositBadgeText(deposit, t, 'fr'),
      'Dépôt · Médiathèque départementale · retour le 1 septembre 2026',
    );
    assert.equal(depositBadgeText(null, t, 'fr'), null);
    assert.equal(depositBadgeText(undefined, t, 'fr'), null);
  });

  it('formats the English badge', async () => {
    const t = await tFor('en');
    assert.equal(
      depositBadgeText(deposit, t, 'en'),
      'Deposit · Médiathèque départementale · return on September 1, 2026',
    );
  });
});

describe('deposit catalog filter', () => {
  const own = { items: [{ deposit: null }] };
  const lent = { items: [{ deposit }] };
  const mixed = { items: [{ deposit: null }, { deposit }] };

  it('keeps own stock and deposits apart on the loaded page', () => {
    assert.equal(biblioMatchesDepositFilter(own, 'own'), true);
    assert.equal(biblioMatchesDepositFilter(lent, 'own'), false);
    assert.equal(biblioMatchesDepositFilter(lent, 'deposits'), true);
    assert.equal(biblioMatchesDepositFilter(own, 'deposits'), false);
    assert.equal(biblioMatchesDepositFilter(mixed, 'own'), true);
    assert.equal(biblioMatchesDepositFilter(mixed, 'deposits', '3'), true);
    assert.equal(biblioMatchesDepositFilter(mixed, 'deposits', '9'), false);
    assert.equal(biblioMatchesDepositFilter(lent, 'all'), true);
  });
});

describe('deposit import payload', () => {
  it('sends deposit_batch_id and drops site and duplicate options', () => {
    assert.deepEqual(
      buildMarcBatchImportParams({
        batchId: '42',
        sourceId: '7',
        depositBatchId: '99',
        recordId: 3,
        allowDuplicateIsbn: true,
        confirmReplaceExistingId: '5',
      }),
      { batchId: '42', recordId: 3, deposit_batch_id: '99' },
    );
  });

  it('keeps the own-stock query', () => {
    assert.deepEqual(
      buildMarcBatchImportParams({
        batchId: '42',
        sourceId: '7',
        allowDuplicateIsbn: true,
        confirmReplaceExistingId: '5',
      }),
      { batchId: '42', sourceId: '7', allowDuplicateIsbn: true, confirmReplaceExistingId: '5' },
    );
  });

  it('posts that payload through the API client', async () => {
    const calls: Array<{ url: string; params: Record<string, unknown> }> = [];
    const client = (api as unknown as {
      client: { post: (url: string, data?: unknown, config?: { params?: Record<string, unknown> }) => Promise<{ data: { taskId: string } }> };
    }).client;
    const original = client.post;
    client.post = (async (url: string, _data: unknown, config?: { params?: Record<string, unknown> }) => {
      calls.push({ url, params: config?.params ?? {} });
      return { data: { taskId: 'task-1' } };
    }) as typeof client.post;
    try {
      await api.importMarcBatch('42', '7', {
        depositBatchId: '99',
        recordId: 3,
        allowDuplicateIsbn: true,
        confirmReplaceExistingId: '5',
      });
    } finally {
      client.post = original;
    }
    assert.equal(calls.length, 1);
    assert.equal(calls[0].url, '/biblios/import-marc-batch');
    assert.deepEqual(calls[0].params, { batchId: '42', recordId: 3, deposit_batch_id: '99' });
  });
});

describe('capped due date', () => {
  it('shows the deposit return notice only when the flag is true', async () => {
    const t = await tFor('fr');
    const notice = depositDueDateNotice(
      { dueDateCappedByDeposit: true, expiryAt: '2026-09-01' },
      t,
      'fr',
    );
    assert.equal(notice, 'Échéance ramenée au 1 septembre 2026 (retour du dépôt)');
    assert.equal(
      withDepositDueNotice('Prêt enregistré', { dueDateCappedByDeposit: true, expiryAt: '2026-09-01' }, t, 'fr'),
      'Prêt enregistré Échéance ramenée au 1 septembre 2026 (retour du dépôt)',
    );
    assert.equal(depositDueDateNotice({ dueDateCappedByDeposit: false, expiryAt: '2026-09-01' }, t, 'fr'), null);
    assert.equal(depositDueDateNotice({ expiryAt: '2026-09-01' }, t, 'fr'), null);
  });
});

describe('deposit error mappings', () => {
  it('refuses force on deposit_return_due and deposit_renewal_no_extension', async () => {
    const t = await tFor('fr');
    const checkout = axiosError(422, 'deposit_return_due', 'past the deposit', { forceable: true });
    assert.deepEqual(checkoutDeskActions('deposit_return_due', true, true), { force: false, setAside: false });
    assert.equal(checkoutMayForce(checkout, true), false);
    assert.equal(
      getApiErrorMessage(checkout, t),
      'Cet exemplaire doit revenir chez le prêteur. Le prêt ne peut pas dépasser la date de retour du dépôt.',
    );

    const renewal = axiosError(422, 'deposit_renewal_no_extension', 'no extension', { forceable: true });
    const view = presentRenewalRefusal(renewal, { alreadyForced: false, t });
    assert.equal(view.kind, 'sentence');
    if (view.kind === 'sentence') {
      assert.equal(
        view.message,
        'Le renouvellement n’allongerait pas l’échéance : elle est déjà à la date de retour du dépôt.',
      );
    }
  });

  it('maps hold, weeding, and import failures', async () => {
    const t = await tFor('en');
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_batch_returning'), t),
      'This deposit is being returned. New holds on its copies are refused.',
    );
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_item_not_owned'), t),
      'This copy belongs to a deposit and cannot be weeded or withdrawn.',
    );
    assert.equal(
      marcImportFailureMessage({ code: 'barcode_already_exists', error: 'An item with barcode X already exists.' }, t),
      'This barcode is already used by another copy.',
    );
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_site_mismatch'), t),
      'The chosen site does not match the deposit batch site.',
    );
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_batch_not_active'), t),
      'This deposit batch is not active.',
    );
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_import_options_conflict'), t),
      'Duplicate ISBN options cannot be used when importing a deposit.',
    );
    assert.equal(
      lenderNameConflictMessage(axiosError(409, 'conflict', 'A lender with this name already exists.'), t),
      'A lender with this name already exists.',
    );
  });
});

describe('deposit close report', () => {
  it('keeps barcode and title and drops reader identity', () => {
    const report = parseDepositCloseReport({
      returned: [{ barcode: 'A1', title: 'Title A', readerName: 'Hidden reader', userId: '9' }],
      missing: [{ barcode: 'B2', title: 'Title B', borrower: { name: 'Also hidden' } }],
    });
    assert.deepEqual(report, {
      returned: [{ barcode: 'A1', title: 'Title A' }],
      missing: [{ barcode: 'B2', title: 'Title B' }],
    });
    assert.equal(JSON.stringify(report).includes('Hidden'), false);
    assert.equal(depositScanAlreadyReturned({ alreadyScanned: true }, false), true);
    assert.equal(depositScanAlreadyReturned({}, true), true);
    assert.equal(depositScanAlreadyReturned({}, false), false);
  });
});
