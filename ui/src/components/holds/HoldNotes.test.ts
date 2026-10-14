import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import { HoldNoteFields, HoldNotes } from './HoldNotes.tsx';

const PATRON_NOTE = 'please keep the CD';
const INTERNAL_NOTE = 'desk only secret';

async function i18nFor(lng: 'fr' | 'en'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    fallbackLng: 'fr',
    resources: {
      fr: { translation: fr },
      en: { translation: en },
      de: { translation: de },
      es: { translation: es },
    },
    interpolation: { escapeValue: false },
  });
  return i18n;
}

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nextProvider, { i18n }, node));
}

describe('hold note form rights', () => {
  it('shows both textareas when the user has holds write', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(HoldNoteFields, {
        patronNote: PATRON_NOTE,
        internalNote: INTERNAL_NOTE,
        onPatronNoteChange: () => {},
        onInternalNoteChange: () => {},
        canEditInternal: true,
      }),
    );
    assert.match(html, /Note visible par le lecteur/);
    assert.match(html, /Note interne \(personnel uniquement\)/);
    assert.match(html, /18\/4000/);
    assert.match(html, /16\/4000/);
    assert.equal((html.match(/<textarea/g) ?? []).length, 2);
  });

  it('uses the English labels on the staff form', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(HoldNoteFields, {
        patronNote: '',
        internalNote: '',
        onPatronNoteChange: () => {},
        onInternalNoteChange: () => {},
        canEditInternal: true,
      }),
    );
    assert.match(html, /Note visible to the reader/);
    assert.match(html, /Internal note \(staff only\)/);
    assert.match(html, /0\/4000/);
    assert.equal((html.match(/<textarea/g) ?? []).length, 2);
  });

  it('hides the internal note from a reader, who can still write their remark', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(HoldNoteFields, {
        patronNote: PATRON_NOTE,
        internalNote: INTERNAL_NOTE,
        onPatronNoteChange: () => {},
        onInternalNoteChange: () => {},
        canEditInternal: false,
      }),
    );
    assert.match(html, /Note visible par le lecteur/);
    assert.match(html, new RegExp(PATRON_NOTE));
    assert.equal((html.match(/<textarea/g) ?? []).length, 1);
    assert.equal(html.includes(INTERNAL_NOTE), false);
    assert.equal(html.includes('Note interne'), false);
    assert.equal(html.includes('personnel uniquement'), false);
  });
});

describe('hold note display', () => {
  it('shows both notes on a staff queue row', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(HoldNotes, {
        audience: 'staff',
        patronNote: PATRON_NOTE,
        internalNote: INTERNAL_NOTE,
      }),
    );
    assert.match(html, /Note visible to the reader/);
    assert.match(html, /Internal note \(staff only\)/);
    assert.match(html, new RegExp(PATRON_NOTE));
    assert.match(html, new RegExp(INTERNAL_NOTE));
  });

  it('shows only the reader note on /me/holds and ignores a leaked internal note', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(HoldNotes, {
        audience: 'reader',
        patronNote: PATRON_NOTE,
        internalNote: INTERNAL_NOTE,
      }),
    );
    assert.match(html, /Note visible par le lecteur/);
    assert.match(html, new RegExp(PATRON_NOTE));
    assert.equal(html.includes(INTERNAL_NOTE), false);
    assert.equal(html.includes('Note interne'), false);
  });

  it('renders nothing when notes are null, blank, or absent', async () => {
    const i18n = await i18nFor('en');
    for (const props of [
      { audience: 'staff' as const, patronNote: null, internalNote: null },
      { audience: 'staff' as const, patronNote: '   ', internalNote: undefined },
      { audience: 'reader' as const, patronNote: null, internalNote: INTERNAL_NOTE },
      { audience: 'reader' as const },
    ]) {
      assert.equal(render(i18n, createElement(HoldNotes, props)), '');
    }
  });
});
