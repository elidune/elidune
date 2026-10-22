import assert from 'node:assert/strict';
import test from 'node:test';

import {
  isReadyToCirculate,
  missingCirculationFields,
  recommendedCirculationFields,
} from './circulationReadiness';

test('circulation gate requires barcode and site only', () => {
  assert.deepEqual(missingCirculationFields({ barcode: 'B1', siteId: '1' }), []);
  assert.equal(isReadyToCirculate({ barcode: 'B1', siteId: '1' }), true);
  assert.equal(isReadyToCirculate({ barcode: 'B1', siteId: '1', price: null }), true);
});

test('missing price is recommended not blocking', () => {
  assert.deepEqual(recommendedCirculationFields({ barcode: 'B1', siteId: '1' }), ['price']);
  assert.deepEqual(recommendedCirculationFields({ barcode: 'B1', siteId: '1', price: '2.00' }), []);
  assert.deepEqual(recommendedCirculationFields({ barcode: 'B1', siteId: '1', priceDeferred: true }), []);
});
