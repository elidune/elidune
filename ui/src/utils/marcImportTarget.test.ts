import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  marcImportAllowsDuplicateIsbn,
  marcImportApiSiteId,
  marcImportDepositBatchId,
  marcImportTargetReady,
} from './marcImportTarget.ts';

describe('marcImportTarget', () => {
  it('requires source or batch in catalog mode', () => {
    assert.equal(
      marcImportTargetReady({
        kind: 'catalog',
        stockMode: 'own',
        selectedSiteId: null,
        selectedDepositBatchId: '',
      }),
      false,
    );
    assert.equal(
      marcImportTargetReady({
        kind: 'catalog',
        stockMode: 'deposit',
        selectedSiteId: null,
        selectedDepositBatchId: 'b1',
      }),
      true,
    );
  });

  it('requires batch site confirmation when several sources exist', () => {
    const sites = [
      { id: '1', name: 'A', key: 'a', default: false },
      { id: '2', name: 'B', key: 'b', default: false },
    ];
    assert.equal(
      marcImportTargetReady({
        kind: 'depositBatch',
        depositBatchId: '10',
        batchSiteId: '2',
        selectedSiteId: null,
        sites,
      }),
      false,
    );
    assert.equal(
      marcImportTargetReady({
        kind: 'depositBatch',
        depositBatchId: '10',
        batchSiteId: '2',
        selectedSiteId: '2',
        sites,
      }),
      true,
    );
  });

  it('builds API ids per situation', () => {
    assert.equal(
      marcImportDepositBatchId({
        kind: 'catalog',
        stockMode: 'deposit',
        selectedSiteId: null,
        selectedDepositBatchId: '99',
      }),
      '99',
    );
    assert.equal(
      marcImportApiSiteId({
        kind: 'depositBatch',
        depositBatchId: '10',
        batchSiteId: '2',
        selectedSiteId: '2',
        sites: [{ id: '1', name: 'A', key: 'a', default: false }, { id: '2', name: 'B', key: 'b', default: false }],
      }),
      '2',
    );
    assert.equal(
      marcImportAllowsDuplicateIsbn({
        kind: 'depositBatch',
        depositBatchId: '10',
        batchSiteId: '2',
        selectedSiteId: '2',
        sites: [],
      }),
      false,
    );
  });
});
