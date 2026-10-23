import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { accountTypeVisualKind } from './userAccountTypeVisual';

describe('accountTypeVisualKind', () => {
  it('maps built-in slugs and legacy digits', () => {
    assert.equal(accountTypeVisualKind('admin'), 'admin');
    assert.equal(accountTypeVisualKind('Administrator'), 'admin');
    assert.equal(accountTypeVisualKind('4'), 'admin');
    assert.equal(accountTypeVisualKind('librarian'), 'librarian');
    assert.equal(accountTypeVisualKind('3'), 'librarian');
    assert.equal(accountTypeVisualKind('group'), 'group');
    assert.equal(accountTypeVisualKind('8'), 'group');
    assert.equal(accountTypeVisualKind('reader'), 'reader');
    assert.equal(accountTypeVisualKind('guest'), 'guest');
    assert.equal(accountTypeVisualKind('volunteer'), 'default');
  });
});
