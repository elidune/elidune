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
  activeLoanCopyLabel,
  depositActiveLoanCopies,
  depositBadgeText,
  depositLenderInitials,
  depositDueDateNotice,
  depositReplacementAmountRequired,
  depositScanAlreadyReturned,
  lenderNameConflictMessage,
  marcImportFailureMessage,
  parseDepositCloseReport,
  parseReplacementRateEuros,
  replacementAmountFieldState,
  returnToLenderNotice,
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
    assert.equal(
      depositBadgeText(deposit, t, 'fr', 'public'),
      'Dépôt · Médiathèque départementale',
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
  it('sends deposit_batch_id and drops duplicate options', () => {
    assert.deepEqual(
      buildMarcBatchImportParams({
        batchId: '42',
        siteId: '7',
        depositBatchId: '99',
        recordId: 3,
        allowDuplicateIsbn: true,
        confirmReplaceExistingId: '5',
      }),
      { batchId: '42', recordId: 3, deposit_batch_id: '99', siteId: '7' },
    );
  });

  it('forwards duplicate resolution params in deposit mode', () => {
    assert.deepEqual(
      buildMarcBatchImportParams({
        batchId: '42',
        depositBatchId: '99',
        depositDuplicateAction: 'overwrite',
        depositDuplicateApplyAll: true,
      }),
      {
        batchId: '42',
        deposit_batch_id: '99',
        depositDuplicateAction: 'overwrite',
        depositDuplicateApplyAll: true,
      },
    );
  });

  it('keeps the own-stock query', () => {
    assert.deepEqual(
      buildMarcBatchImportParams({
        batchId: '42',
        siteId: '7',
        allowDuplicateIsbn: true,
        confirmReplaceExistingId: '5',
      }),
      { batchId: '42', siteId: '7', allowDuplicateIsbn: true, confirmReplaceExistingId: '5' },
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
    assert.deepEqual(calls[0].params, { batchId: '42', recordId: 3, deposit_batch_id: '99', siteId: '7' });
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
      getApiErrorMessage(
        axiosError(
          422,
          'deposit_item_on_loan',
          'Check this copy in at the desk before scanning it out to the lender. It still has an open loan',
        ),
        t,
      ),
      'Check this copy in at the desk before scanning it out to the lender. It still has an open loan.',
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
      onLoan: [{ barcode: 'C3', title: 'Title C', userName: 'Still hidden', cardNumber: '100' }],
    });
    assert.deepEqual(report, {
      returned: [{ barcode: 'A1', title: 'Title A' }],
      missing: [{ barcode: 'B2', title: 'Title B' }],
    });
    assert.equal(JSON.stringify(report).includes('Hidden'), false);
    assert.equal(JSON.stringify(report).includes('Title C'), false);
    assert.equal(JSON.stringify(report).includes('onLoan'), false);
    assert.equal(depositScanAlreadyReturned({ alreadyScanned: true }, false), true);
    assert.equal(depositScanAlreadyReturned({}, true), true);
    assert.equal(depositScanAlreadyReturned({}, false), false);
  });

  it('reads the copies still on loan from a close refusal and ignores reader names', async () => {
    const t = await tFor('fr');
    const error = axiosError(422, 'deposit_batch_has_active_loans', 'copies still out', {
      copies: [
        { itemId: '44', barcode: 'L1', title: 'Not from the error', userName: 'Lecteur secret' },
      ],
    });
    assert.deepEqual(depositActiveLoanCopies(error), [
      { itemId: '44', barcode: 'L1' },
    ]);
    assert.equal(JSON.stringify(depositActiveLoanCopies(error)).includes('secret'), false);
    assert.equal(JSON.stringify(depositActiveLoanCopies(error)).includes('Not from'), false);
    assert.deepEqual(
      activeLoanCopyLabel(
        { itemId: '44', barcode: 'L1' },
        [{ itemId: '44', title: 'Encore dehors', biblioId: '9' }],
      ),
      { title: 'Encore dehors', biblioId: '9' },
    );
    assert.deepEqual(
      activeLoanCopyLabel({ itemId: '44', barcode: null }, []),
      { title: null, biblioId: null },
    );
    assert.equal(depositActiveLoanCopies(axiosError(422, 'deposit_batch_not_returning')), null);
    assert.equal(
      getApiErrorMessage(error, t),
      'Le lot reste en retour : des exemplaires sont encore en prêt.',
    );
    assert.equal(
      getApiErrorMessage(axiosError(422, 'deposit_batch_not_returning'), t),
      'Le lot n’est pas en retour. Lancez le retour avant de scanner une sortie ou de clôturer.',
    );
  });
});

describe('return to lender notice', () => {
  it('names the lender and stays silent when the flag is false', async () => {
    const fr = await tFor('fr');
    const en = await tFor('en');
    const body = {
      status: 'returned',
      returnToLender: true,
      loan: {
        itemId: '4',
        user: { firstname: 'Hidden', lastname: 'Reader' },
        biblio: {
          items: [{ id: '4', deposit: { lenderName: 'Médiathèque départementale' } }],
        },
      },
    };
    assert.equal(
      returnToLenderNotice(body, fr),
      'À renvoyer au prêteur (Médiathèque départementale) — ne pas remettre en rayon',
    );
    assert.equal(
      returnToLenderNotice({ returnToLender: true }, en, 'County library'),
      'Send back to the lender (County library) — do not shelve',
    );
    assert.equal(
      returnToLenderNotice({ returnToLender: true }, fr),
      'À renvoyer au prêteur — ne pas remettre en rayon',
    );
    assert.equal(returnToLenderNotice({ returnToLender: false, lenderName: 'Médiathèque' }, fr), null);
    assert.equal(returnToLenderNotice({ alreadyScanned: true }, fr), null);
  });
});

describe('deposit replacement amount', () => {
  it('requires a positive staff amount after the server refusal', async () => {
    const t = await tFor('en');
    const error = axiosError(422, 'deposit_replacement_amount_required');
    assert.equal(depositReplacementAmountRequired(error), true);
    assert.equal(depositReplacementAmountRequired(axiosError(422, 'deposit_return_due')), false);
    assert.equal(
      getApiErrorMessage(error, t),
      'Enter the replacement amount. Order: lender rate, then the copy price when it is greater than zero, otherwise the amount you enter. Zero is not accepted.',
    );
    assert.deepEqual(
      replacementAmountFieldState({ kind: 'lost', bill: false, amount: '', serverRequiresAmount: true }),
      { bill: true, required: true, invalid: true },
    );
    assert.equal(
      replacementAmountFieldState({ kind: 'damaged', bill: false, amount: '0', serverRequiresAmount: true }).invalid,
      true,
    );
    assert.equal(
      replacementAmountFieldState({ kind: 'lost', bill: false, amount: '12,50', serverRequiresAmount: true }).invalid,
      false,
    );
    assert.equal(
      replacementAmountFieldState({ kind: 'lost', bill: false, amount: '', serverRequiresAmount: false }).required,
      false,
    );
    assert.deepEqual(parseReplacementRateEuros(''), { ok: true, cents: null });
    assert.deepEqual(parseReplacementRateEuros('12,50'), { ok: true, cents: 1250 });
    assert.deepEqual(parseReplacementRateEuros('0'), { ok: false, reason: 'notPositive' });
  });

  it('depositLenderInitials', () => {
    assert.equal(depositLenderInitials('Médiathèque départementale'), 'MD');
    assert.equal(depositLenderInitials('Smith'), 'SM');
    assert.equal(depositLenderInitials(''), '?');
  });
});
