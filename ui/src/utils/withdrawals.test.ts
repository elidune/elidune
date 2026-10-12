import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import i18next from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import {
  addScannedWithdrawalCopy,
  canCancelWithdrawal,
  canCompleteLegacyWithdrawal,
  canMarkDisposed,
  csvExportLanguage,
  withdrawalExportFilename,
  exportPeriodError,
  localDayBoundIso,
  parseDisposeFailures,
  toWithdrawalListParams,
  validateWithdrawalWrite,
  emptyWithdrawalFilters,
} from './withdrawals.ts';

describe('withdrawal write validation', () => {
  it('requires a destination and a 3-character detail when the reason is other', () => {
    assert.deepEqual(
      validateWithdrawalWrite({ reason: '', reasonDetail: '', destination: 'pulp' }),
      { ok: false, field: 'reason' },
    );
    assert.deepEqual(
      validateWithdrawalWrite({ reason: 'other', reasonDetail: '  ab ', destination: 'pulp' }),
      { ok: false, field: 'detail' },
    );
    assert.deepEqual(
      validateWithdrawalWrite({ reason: 'worn', reasonDetail: '', destination: '' }),
      { ok: false, field: 'destination' },
    );
    assert.deepEqual(
      validateWithdrawalWrite({ reason: 'other', reasonDetail: '  spine  ', destination: 'donation' }),
      {
        ok: true,
        body: { reason: 'other', reasonDetail: 'spine', destination: 'donation' },
      },
    );
  });

  it('sends a null detail when a non-other reason has no text', () => {
    assert.deepEqual(
      validateWithdrawalWrite({ reason: 'surplus', reasonDetail: '   ', destination: 'sale' }),
      { ok: true, body: { reason: 'surplus', reasonDetail: null, destination: 'sale' } },
    );
  });
});

describe('disposal state', () => {
  const active = { cancelledAt: null, disposedAt: null, legacy: false };
  const disposed = { cancelledAt: null, disposedAt: '2026-01-01T00:00:00Z', legacy: true };

  it('refuses cancel and dispose once disposedAt is set, and still allows legacy completion', () => {
    assert.equal(canCancelWithdrawal(active), true);
    assert.equal(canCancelWithdrawal(disposed), false);
    assert.equal(canMarkDisposed(disposed), false);
    assert.equal(canCompleteLegacyWithdrawal(disposed), true);
    assert.equal(canCompleteLegacyWithdrawal({ ...active, legacy: false }), false);
  });
});

describe('dispose batch failures', () => {
  it('reads per-id codes and ignores incomplete rows', () => {
    assert.deepEqual(
      parseDisposeFailures({
        code: 'withdrawal_dispose_rejected',
        failures: [
          { id: '9002', code: 'withdrawal_not_active', message: 'cancelled' },
          { id: '9003', code: 'withdrawal_already_disposed', message: 'disposed' },
          { id: 9999, code: 'not_found', message: 'missing' },
          { code: 'not_found' },
        ],
      }),
      [
        { id: '9002', code: 'withdrawal_not_active', message: 'cancelled' },
        { id: '9003', code: 'withdrawal_already_disposed', message: 'disposed' },
        { id: '9999', code: 'not_found', message: 'missing' },
      ],
    );
    assert.deepEqual(parseDisposeFailures({ code: 'validation_error' }), []);
    assert.deepEqual(parseDisposeFailures(null), []);
  });
});

describe('list and export params', () => {
  it('omits disposed for all, and passes true or false otherwise', () => {
    const base = emptyWithdrawalFilters();
    assert.equal(toWithdrawalListParams(base).disposed, undefined);
    assert.equal(toWithdrawalListParams({ ...base, disposed: 'yes' }).disposed, true);
    assert.equal(toWithdrawalListParams({ ...base, disposed: 'no' }).disposed, false);
  });

  it('drops pagination on export and keeps the disposed filter', () => {
    const params = toWithdrawalListParams(
      { ...emptyWithdrawalFilters(), from: '2026-01-02', to: '2026-01-02', disposed: 'no' },
      { forExport: true, lang: 'french' },
    );
    assert.equal(params.page, undefined);
    assert.equal(params.perPage, undefined);
    assert.equal(params.disposed, false);
    assert.equal(params.lang, 'french');
    assert.equal(typeof params.from, 'string');
    assert.ok(params.from!.startsWith('2026-01-'));
  });

  it('rejects an export period that is missing or reversed', () => {
    assert.equal(exportPeriodError('', '2026-01-02'), 'missing');
    assert.equal(exportPeriodError('2026-02-01', '2026-01-01'), 'order');
    assert.equal(exportPeriodError('2026-01-01', '2026-01-01'), null);
  });

  it('maps the UI language onto the CSV lang query', () => {
    assert.equal(csvExportLanguage('fr-FR'), 'french');
    assert.equal(csvExportLanguage('en'), 'english');
    assert.equal(csvExportLanguage('de'), 'english');
  });

  it('names the export file from the period, in the UI language', () => {
    assert.equal(
      withdrawalExportFilename('fr', '2026-01-02', '2026-03-04'),
      'retraits_2026-01-02_2026-03-04.csv',
    );
    assert.equal(
      withdrawalExportFilename('en-GB', '2026-01-02', '2026-03-04'),
      'withdrawals_2026-01-02_2026-03-04.csv',
    );
    assert.equal(
      withdrawalExportFilename('de', '2026-01-02', '2026-03-04'),
      'aussonderungen_2026-01-02_2026-03-04.csv',
    );
    assert.equal(
      withdrawalExportFilename('es', '2026-01-02', '2026-03-04'),
      'expurgos_2026-01-02_2026-03-04.csv',
    );
  });

  it('keeps copies from different biblios and skips a duplicate item', () => {
    const first = addScannedWithdrawalCopy([], {
      itemId: '1',
      biblioId: '10',
      barcode: 'A',
      title: 'One',
    });
    const second = addScannedWithdrawalCopy(first.list, {
      itemId: '2',
      biblioId: '20',
      barcode: 'B',
      title: 'Two',
    });
    const again = addScannedWithdrawalCopy(second.list, {
      itemId: '1',
      biblioId: '10',
      barcode: 'A',
      title: 'One',
    });
    assert.equal(first.duplicate, false);
    assert.equal(second.duplicate, false);
    assert.equal(again.duplicate, true);
    assert.deepEqual(
      second.list.map((row) => row.itemId),
      ['1', '2'],
    );
    assert.deepEqual(
      again.list.map((row) => row.biblioId),
      ['10', '20'],
    );
  });

  it('rejects a calendar date that does not exist', () => {
    assert.equal(localDayBoundIso('2026-02-31', 'start'), null);
    assert.equal(localDayBoundIso('2026-02-01', 'start')?.includes('T'), true);
  });
});

const WEEDING_PLURAL_KEYS = ['listCount', 'dialogTitleBatch', 'markDisposedConfirm'] as const;
const WEEDING_SCREEN_KEYS = [
  'detailTitle',
  'backToList',
  'scanTitle',
  'scanHint',
  'scanPlaceholder',
  'scanAdd',
  'scanDuplicate',
  'scanEmpty',
  'scanRemove',
  'scanNotFound',
  'scanAlreadyWithdrawn',
] as const;

describe('weeding plural copy', () => {
  const catalogs = { en, fr, de, es };

  it('stores counters and confirmations as i18next one/other forms', () => {
    for (const catalog of Object.values(catalogs)) {
      const weeding = catalog.weeding as Record<string, unknown>;
      for (const key of WEEDING_PLURAL_KEYS) {
        assert.equal(key in weeding, false);
        assert.equal(`${key}_plural` in weeding, false);
        assert.equal(typeof weeding[`${key}_one`], 'string');
        assert.equal(typeof weeding[`${key}_other`], 'string');
      }
      assert.equal('detailsUnavailable' in weeding, false);
      for (const key of WEEDING_SCREEN_KEYS) {
        assert.equal(typeof weeding[key], 'string');
        assert.ok(String(weeding[key]).length > 0);
      }
    }
  });

  it('resolves French, English, German, and Spanish counts', async () => {
    const i18n = i18next.createInstance();
    await i18n.init({
      lng: 'en',
      fallbackLng: 'en',
      resources: {
        en: { translation: en },
        fr: { translation: fr },
        de: { translation: de },
        es: { translation: es },
      },
      interpolation: { escapeValue: false },
    });

    const expected: Array<[string, string, number, string]> = [
      ['fr', 'weeding.listCount', 0, '0 retrait'],
      ['fr', 'weeding.listCount', 1, '1 retrait'],
      ['fr', 'weeding.listCount', 2, '2 retraits'],
      ['fr', 'weeding.dialogTitleBatch', 1, 'Retirer 1 exemplaire'],
      ['fr', 'weeding.dialogTitleBatch', 3, 'Retirer 3 exemplaires'],
      ['en', 'weeding.listCount', 1, '1 withdrawal'],
      ['en', 'weeding.listCount', 0, '0 withdrawals'],
      ['en', 'weeding.dialogTitleBatch', 1, 'Withdraw 1 copy'],
      ['en', 'weeding.dialogTitleBatch', 2, 'Withdraw 2 copies'],
      ['de', 'weeding.listCount', 1, '1 Aussonderung'],
      ['de', 'weeding.listCount', 2, '2 Aussonderungen'],
      ['de', 'weeding.dialogTitleBatch', 1, '1 Exemplar aussondern'],
      ['es', 'weeding.listCount', 1, '1 expurgo'],
      ['es', 'weeding.listCount', 2, '2 expurgos'],
      ['es', 'weeding.dialogTitleBatch', 1, 'Retirar 1 ejemplar'],
    ];

    for (const [lng, key, count, text] of expected) {
      assert.equal(i18n.t(key, { lng, count }), text);
    }
    assert.match(String(i18n.t('weeding.markDisposedConfirm', { lng: 'fr', count: 1 })), /1 retrait/);
    assert.match(String(i18n.t('weeding.markDisposedConfirm', { lng: 'fr', count: 2 })), /2 retraits/);
    assert.match(String(i18n.t('weeding.markDisposedConfirm', { lng: 'en', count: 1 })), /1 withdrawal/);
    assert.match(String(i18n.t('weeding.markDisposedConfirm', { lng: 'en', count: 2 })), /2 withdrawals/);
  });
});
