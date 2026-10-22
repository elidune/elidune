import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  isFeatureEnabled,
  resolveEnabledFeatures,
  effectiveStaffAccess,
} from './featureAccess.ts';

describe('resolveEnabledFeatures', () => {
  it('defaults all modules to enabled when absent', () => {
    const f = resolveEnabledFeatures(undefined);
    assert.equal(f.acquisitions, true);
    assert.equal(f.purchaseSuggestions, true);
    assert.equal(f.weeding, true);
  });

  it('fills missing keys as enabled', () => {
    const f = resolveEnabledFeatures({ acquisitions: false });
    assert.equal(f.acquisitions, false);
    assert.equal(f.events, true);
  });
});

describe('isFeatureEnabled', () => {
  it('returns false when the module is off', () => {
    assert.equal(isFeatureEnabled({ purchaseSuggestions: false }, 'purchaseSuggestions'), false);
  });

  it('returns true when the key is missing', () => {
    assert.equal(isFeatureEnabled({}, 'stats'), true);
  });
});

describe('effectiveStaffAccess', () => {
  it('masks acquisitions when the instance feature is off even if RBAC would allow', () => {
    const user = {
      acquisitionsRights: 'w',
      rights: { acquisitionsRights: 'w' },
    };
    const withoutFeature = effectiveStaffAccess(user as never, null, { acquisitions: false });
    assert.equal(withoutFeature.acquisitions, false);
  });
});
