import assert from 'node:assert/strict';
import test from 'node:test';
import { serialCopyHeading } from './serialCopyCaption.ts';

test('serial copy heading keeps a label that already names the year', () => {
  assert.equal(
    serialCopyHeading({ label: 'n° 412, mars 2026', date: '2026-03-01' }, 'fr'),
    'n° 412, mars 2026',
  );
});

test('serial copy heading appends the cover date when the label has none', () => {
  const heading = serialCopyHeading({ label: 'v. 1 n° 1', date: '2026-03-01' }, 'fr');
  assert.ok(heading?.startsWith('v. 1 n° 1 · '));
  assert.notEqual(heading, 'v. 1 n° 1');
});

test('serial copy heading is empty without a caption', () => {
  assert.equal(serialCopyHeading(null, 'fr'), null);
  assert.equal(serialCopyHeading({ label: '  ', date: '' }, 'fr'), null);
});
