import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  COPY_NOTE_MAX_CHARS,
  clampCopyNote,
  copyNoteLength,
  copyNoteUpdate,
  copyNoteWrite,
} from './itemNotes.ts';

describe('copy note writes', () => {
  it('omits a note the user did not change', () => {
    assert.equal(copyNoteWrite('CD missing', 'CD missing'), undefined);
    assert.equal(copyNoteWrite('CD missing', '  CD missing  '), undefined);
    assert.equal(copyNoteWrite(null, ''), undefined);
    assert.equal(copyNoteWrite(null, '   '), undefined);
    assert.deepEqual(
      copyNoteUpdate(
        { publicNote: 'CD missing', internalNote: 'spine cracked' },
        { publicNote: 'CD missing', internalNote: 'spine cracked' },
      ),
      {},
    );
  });

  it('sends an empty string when the user clears a note', () => {
    assert.equal(copyNoteWrite('CD missing', ''), '');
    assert.equal(copyNoteWrite('CD missing', '   '), '');
    assert.deepEqual(
      copyNoteUpdate(
        { publicNote: 'CD missing', internalNote: 'spine cracked' },
        { publicNote: '', internalNote: 'spine cracked' },
      ),
      { publicNote: '' },
    );
  });

  it('sends a trimmed replacement and leaves the other note out', () => {
    assert.deepEqual(
      copyNoteUpdate(
        { publicNote: null, internalNote: 'provenance' },
        { publicNote: '  2-disc box set  ', internalNote: 'provenance' },
      ),
      { publicNote: '2-disc box set' },
    );
  });

  it('caps a note at 4000 scalar values', () => {
    const over = `${'é'.repeat(COPY_NOTE_MAX_CHARS)}😀`;
    const clamped = clampCopyNote(over);
    assert.equal(copyNoteLength(clamped), COPY_NOTE_MAX_CHARS);
    assert.equal(clamped.endsWith('😀'), false);
    assert.equal(copyNoteWrite(null, over), 'é'.repeat(COPY_NOTE_MAX_CHARS));
  });
});
