import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { withoutPersistentErrorToasts } from './toastPersistence.ts';

describe('persistent error toasts', () => {
  it('drops sticky errors and leaves every other toast', () => {
    const success = { id: 's', variant: 'success' as const, duration: 5_000, message: 'ok' };
    const info = { id: 'i', variant: 'info' as const, duration: 4_000, message: 'note' };
    const timedError = { id: 'e', variant: 'error' as const, duration: 3_000, message: 'brief' };
    const sticky = {
      id: 'lost',
      variant: 'error' as const,
      duration: 0,
      message: 'Carte déclarée perdue',
    };
    const kept = withoutPersistentErrorToasts([success, sticky, info, timedError]);
    assert.deepEqual(kept.map((toast) => toast.id), ['s', 'i', 'e']);
    const staying = [success, info];
    assert.equal(withoutPersistentErrorToasts(staying), staying);
  });
});
