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
import { CopyNoteFields, CopyNotes } from './CopyNotes.tsx';

const PUBLIC_NOTE = 'CD missing';
const INTERNAL_NOTE = 'spine cracked secret';

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

describe('copy note labels', () => {
  it('uses the French public and internal labels on the staff form', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(CopyNoteFields, {
        publicNote: PUBLIC_NOTE,
        internalNote: INTERNAL_NOTE,
        onPublicNoteChange: () => {},
        onInternalNoteChange: () => {},
      }),
    );
    assert.match(html, /Note publique \(visible par les lecteurs\)/);
    assert.match(html, /Note interne \(personnel uniquement\)/);
    assert.match(html, /10\/4000/);
    assert.match(html, /20\/4000/);
    assert.equal(html.includes('<textarea'), true);
    assert.equal((html.match(/<textarea/g) ?? []).length, 2);
  });

  it('uses the English equivalents on the staff form', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(CopyNoteFields, {
        publicNote: '',
        internalNote: '',
        onPublicNoteChange: () => {},
        onInternalNoteChange: () => {},
      }),
    );
    assert.match(html, /Public note \(visible to readers\)/);
    assert.match(html, /Internal note \(staff only\)/);
    assert.match(html, /0\/4000/);
  });
});

describe('copy note display', () => {
  it('shows only the public note on a public or OPAC copy', async () => {
    const html = render(
      await i18nFor('fr'),
      createElement(CopyNotes, {
        audience: 'public',
        publicNote: PUBLIC_NOTE,
        internalNote: INTERNAL_NOTE,
      }),
    );
    assert.match(html, /Note publique \(visible par les lecteurs\)/);
    assert.match(html, new RegExp(PUBLIC_NOTE));
    assert.equal(html.includes(INTERNAL_NOTE), false);
    assert.equal(html.includes('Note interne'), false);
    assert.equal(html.includes('personnel uniquement'), false);
  });

  it('shows both notes on a staff copy', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(CopyNotes, {
        audience: 'staff',
        publicNote: PUBLIC_NOTE,
        internalNote: INTERNAL_NOTE,
      }),
    );
    assert.match(html, /Public note \(visible to readers\)/);
    assert.match(html, /Internal note \(staff only\)/);
    assert.match(html, new RegExp(PUBLIC_NOTE));
    assert.match(html, new RegExp(INTERNAL_NOTE));
  });

  it('renders nothing when the public copy has no public note', async () => {
    const html = render(
      await i18nFor('en'),
      createElement(CopyNotes, {
        audience: 'public',
        publicNote: '   ',
        internalNote: INTERNAL_NOTE,
      }),
    );
    assert.equal(html, '');
  });
});
