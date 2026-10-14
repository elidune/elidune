import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { buildCreateHold } from './holdDisplay.ts';
import {
  HOLD_NOTE_MAX_CHARS,
  INTERNAL_HOLD_NOTE_FORBIDDEN,
  clampHoldNote,
  holdNoteLength,
  holdNotePayload,
  holdNoteText,
  holdNoteWrite,
  holdWriteErrorMessage,
  holdWritePayload,
} from './holdNotes.ts';

function httpError(status: number, code: string, message: string) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: { code, error: status === 403 ? 'Forbidden' : 'Validation Error', message } },
  };
}

describe('hold note writes', () => {
  it('omits a note the user did not change', () => {
    assert.equal(holdNoteWrite('please keep the CD', 'please keep the CD'), undefined);
    assert.equal(holdNoteWrite('please keep the CD', '  please keep the CD  '), undefined);
    assert.equal(holdNoteWrite(null, ''), undefined);
    assert.equal(holdNoteWrite(undefined, '   '), undefined);
    assert.deepEqual(
      holdNotePayload(
        { patronNote: 'please keep the CD', internalNote: 'desk only' },
        { patronNote: 'please keep the CD', internalNote: 'desk only' },
        { canWriteInternal: true },
      ),
      {},
    );
  });

  it('sends an empty string when the user clears a note', () => {
    assert.equal(holdNoteWrite('please keep the CD', ''), '');
    assert.equal(holdNoteWrite('desk only', '   '), '');
    assert.deepEqual(
      holdNotePayload(
        { patronNote: 'please keep the CD', internalNote: 'desk only' },
        { patronNote: '', internalNote: 'desk only' },
        { canWriteInternal: true },
      ),
      { patronNote: '' },
    );
  });

  it('sends a trimmed replacement and leaves the other note out', () => {
    assert.deepEqual(
      holdNotePayload(
        { patronNote: null, internalNote: 'desk only' },
        { patronNote: '  call me  ', internalNote: 'desk only' },
        { canWriteInternal: true },
      ),
      { patronNote: 'call me' },
    );
  });

  it('never sends internalNote without holds write, and never sends notes', () => {
    const reader = holdNotePayload(
      {},
      { patronNote: 'my remark', internalNote: 'staff secret' },
      { canWriteInternal: false },
    );
    assert.deepEqual(reader, { patronNote: 'my remark' });
    assert.equal('internalNote' in reader, false);
    assert.equal('notes' in reader, false);

    const body = holdWritePayload(
      {
        userId: '1',
        biblioId: '2',
        notes: 'legacy field must not be sent',
        patronNote: 'visible',
        internalNote: 'staff secret',
      },
      { canWriteInternal: false },
    );
    assert.equal('notes' in body, false);
    assert.equal(JSON.stringify(body).includes('legacy field must not be sent'), false);
    assert.equal('internalNote' in body, false);
    assert.equal(body.patronNote, 'visible');

    const created = buildCreateHold({
      userId: '1',
      biblioId: '2',
      scope: 'title',
      patronNote: 'my remark',
      internalNote: 'staff secret',
      canWriteInternal: false,
    });
    assert.equal(created.patronNote, 'my remark');
    assert.equal('internalNote' in created, false);
    assert.equal('notes' in created, false);
    assert.equal(JSON.stringify(created).includes('staff secret'), false);
    assert.equal(JSON.stringify(created).includes('notes'), false);
  });

  it('sends internalNote only when holds write changed it', () => {
    const created = buildCreateHold({
      userId: '1',
      biblioId: '2',
      scope: 'copy',
      itemId: '9',
      patronNote: '  ',
      internalNote: ' desk only ',
      canWriteInternal: true,
    });
    assert.equal(created.itemId, '9');
    assert.equal('patronNote' in created, false);
    assert.equal(created.internalNote, 'desk only');
    assert.equal('notes' in created, false);
  });

  it('caps a note at 4000 code points, counting an emoji as one', () => {
    const emoji = '😀';
    assert.equal(emoji.length, 2);
    assert.equal(holdNoteLength(emoji), [...emoji].length);
    assert.equal(holdNoteLength(emoji), 1);

    const over = emoji.repeat(HOLD_NOTE_MAX_CHARS + 1);
    assert.ok(over.length > HOLD_NOTE_MAX_CHARS);
    const clamped = clampHoldNote(over);
    assert.equal([...clamped].length, HOLD_NOTE_MAX_CHARS);
    assert.equal(clamped.length, HOLD_NOTE_MAX_CHARS * 2);
    assert.equal(holdNoteWrite(null, over)?.length, HOLD_NOTE_MAX_CHARS * 2);
    assert.equal([...holdNoteWrite(null, over)!].length, HOLD_NOTE_MAX_CHARS);
  });

  it('treats null, absent, and blank notes as empty display text', () => {
    assert.equal(holdNoteText(null), '');
    assert.equal(holdNoteText(undefined), '');
    assert.equal(holdNoteText('   '), '');
    assert.equal(holdNoteText('keep'), 'keep');
  });
});

describe('hold note permission errors', () => {
  it('shows a clear message for the internal-note 403 and not an empty string', async () => {
    const i18n = createInstance();
    await i18n.init({
      lng: 'fr',
      fallbackLng: 'en',
      resources: {
        fr: { translation: fr },
        en: { translation: en },
        de: { translation: de },
        es: { translation: es },
      },
      interpolation: { escapeValue: false },
    });
    const denied = httpError(403, 'authorization_failed', INTERNAL_HOLD_NOTE_FORBIDDEN);
    const message = holdWriteErrorMessage(denied, i18n.t.bind(i18n));
    assert.equal(message, 'Vous n\'avez pas le droit d\'enregistrer une note interne.');
    assert.notEqual(message, '');
    await i18n.changeLanguage('en');
    assert.equal(
      holdWriteErrorMessage(denied, i18n.t.bind(i18n)),
      'You do not have permission to save an internal hold note.',
    );
    const other = holdWriteErrorMessage(
      httpError(403, 'authorization_failed', 'Cannot cancel another user\'s hold'),
      i18n.t.bind(i18n),
    );
    assert.notEqual(other, '');
    assert.equal(other.includes('internal hold note'), false);
  });
});
