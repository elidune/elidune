import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { navSectionCollapsedVisibleCount } from './sidebarNav';

describe('navSectionCollapsedVisibleCount', () => {
  it('shows all items when count is 2 or less', () => {
    assert.equal(navSectionCollapsedVisibleCount(0), 0);
    assert.equal(navSectionCollapsedVisibleCount(1), 1);
    assert.equal(navSectionCollapsedVisibleCount(2), 2);
  });

  it('shows about two thirds with at least one hidden item', () => {
    assert.equal(navSectionCollapsedVisibleCount(3), 2);
    assert.equal(navSectionCollapsedVisibleCount(4), 2);
    assert.equal(navSectionCollapsedVisibleCount(5), 3);
    assert.equal(navSectionCollapsedVisibleCount(6), 4);
  });
});
