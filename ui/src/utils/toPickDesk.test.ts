import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import type { ItemCheckIn } from '../types/index.ts';
import { interpretPickCheckIn } from './toPickDesk.ts';

describe('interpretPickCheckIn holdFor', () => {
  it('names the reader from holdFor when the hold has no embedded user', () => {
    const ready = interpretPickCheckIn({
      status: 'ready',
      notice: { code: 'item_not_on_loan', level: 'informational' },
      hold: { status: 'ready', userId: '9' },
      holdFor: { id: '9', cardNumber: 'C-9', initials: 'GH' },
    } as ItemCheckIn);
    assert.equal(ready?.kind, 'ready');
    assert.equal(ready?.patronName, 'C-9');
    const named = interpretPickCheckIn({
      status: 'ready',
      holdFor: { id: '9', firstname: 'Grace', lastname: 'Hopper', login: 'gh' },
    } as ItemCheckIn);
    assert.equal(named?.patronName, 'Grace Hopper');
  });
});
