import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  canCancelWithdrawal,
  canCompleteLegacyWithdrawal,
  canMarkDisposed,
  csvExportLanguage,
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

  it('rejects a calendar date that does not exist', () => {
    assert.equal(localDayBoundIso('2026-02-31', 'start'), null);
    assert.equal(localDayBoundIso('2026-02-01', 'start')?.includes('T'), true);
  });
});
