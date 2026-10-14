import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import {
  COPY_NOTE_MAX_CHARS,
  clampCopyNote,
  copyNoteLength,
  copyNoteUpdate,
  copyNoteWrite,
  biblioWritePayload,
  itemWritePayload,
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

  it('caps a note at 4000 code points, not UTF-16 units', () => {
    const emoji = '😀';
    assert.equal(emoji.length, 2);
    assert.equal(copyNoteLength(emoji), [...emoji].length);
    assert.equal(copyNoteLength(emoji), 1);

    const over = emoji.repeat(COPY_NOTE_MAX_CHARS + 1);
    assert.ok(over.length > COPY_NOTE_MAX_CHARS);
    const clamped = clampCopyNote(over);
    assert.equal([...clamped].length, COPY_NOTE_MAX_CHARS);
    assert.equal(clamped.length, COPY_NOTE_MAX_CHARS * 2);
    assert.equal(copyNoteWrite(null, over)?.length, COPY_NOTE_MAX_CHARS * 2);
  });

  it('never sends the retired notes field', () => {
    const body = itemWritePayload({
      barcode: 'COPY',
      notes: 'legacy field must not be sent',
      publicNote: `${'😀'.repeat(COPY_NOTE_MAX_CHARS)}x`,
      internalNote: '',
    });
    assert.equal('notes' in body, false);
    assert.equal(JSON.stringify(body).includes('legacy field must not be sent'), false);
    assert.equal([...(body.publicNote ?? '')].length, COPY_NOTE_MAX_CHARS);
    assert.equal(body.internalNote, '');
  });

  it('strips notes on nested copies and keeps the bibliographic note', () => {
    const body = biblioWritePayload({
      title: 'Notice',
      notes: 'bibliographic note',
      items: [
        { barcode: 'A', notes: 'copy note', publicNote: 'CD missing' },
        { barcode: 'B', callNumber: '800' },
      ],
    });
    assert.equal(body.notes, 'bibliographic note');
    assert.equal('notes' in body.items[0], false);
    assert.equal(body.items[0].publicNote, 'CD missing');
    assert.equal('notes' in body.items[1], false);
    assert.equal(body.items[1].callNumber, '800');
  });
});
