import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { PUBLIC_LANGUAGE_OPTIONS } from '../locales/languages.ts';
import type { User } from '../types/index.ts';
import { appendReaderLanguage, readerLanguageSelectValue } from './readerLanguage.ts';

if (typeof globalThis.localStorage === 'undefined') {
  const memoryStore = new Map<string, string>();
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => memoryStore.get(key) ?? null,
      setItem: (key: string, value: string) => {
        memoryStore.set(key, value);
      },
      removeItem: (key: string) => {
        memoryStore.delete(key);
      },
      clear: () => {
        memoryStore.clear();
      },
    },
  });
}

const LIBRARY_OPTION = {
  fr: 'Langue de la bibliothèque',
  en: 'Library language',
  de: 'Sprache der Bibliothek',
  es: 'Idioma de la biblioteca',
} as const;

async function i18nFor(lng: keyof typeof LIBRARY_OPTION): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
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

async function renderForm(
  i18n: I18nInstance,
  props: Record<string, unknown> = {},
): Promise<string> {
  const { default: UserEditorForm } = await import('../components/users/UserEditorForm.tsx');
  const client = new QueryClient();
  return renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client },
      createElement(
        I18nextProvider,
        { i18n },
        createElement(UserEditorForm, {
          mode: 'create',
          formId: 'new-reader',
          publicTypes: [],
          accountTypes: [],
          onLoadingChange: () => undefined,
          onSuccess: () => undefined,
          ...props,
        }),
      ),
    ) as ReactElement,
  );
}

/** Same JSON the POST body uses: omitted and undefined fields are absent. */
function requestBody(choice: string): Record<string, unknown> {
  return JSON.parse(JSON.stringify(appendReaderLanguage({ login: 'ada' }, choice))) as Record<string, unknown>;
}

describe('reader language on POST /users', () => {
  it('create without picking sends no language field', () => {
    const body = requestBody('');
    assert.equal('language' in body, false);
    assert.equal(body.language, undefined);
  });

  it('picking de sends language de', () => {
    const body = requestBody('de');
    assert.equal(body.language, 'de');
  });

  it('does not default a new reader to the UI locale', () => {
    for (const locale of ['fr', 'en', 'de', 'es'] as const) {
      const body = requestBody(readerLanguageSelectValue(undefined));
      assert.equal('language' in body, false);
      assert.notEqual(body.language, locale);
    }
  });

  it('keeps an existing reader language and leaves an unset one empty', () => {
    assert.equal(readerLanguageSelectValue('german'), 'de');
    assert.equal(readerLanguageSelectValue('de'), 'de');
    assert.equal(requestBody(readerLanguageSelectValue('german')).language, 'de');
    assert.equal(readerLanguageSelectValue(null), '');
    assert.equal(readerLanguageSelectValue(''), '');
    assert.equal('language' in requestBody(readerLanguageSelectValue(null)), false);
  });

  it('only sends the supported UI codes', () => {
    for (const code of PUBLIC_LANGUAGE_OPTIONS) {
      assert.equal(requestBody(code).language, code);
    }
    assert.equal('language' in requestBody('french'), false);
    assert.equal('language' in requestBody('italian'), false);
  });
});

describe('reader language select', () => {
  it('defaults a new reader to the library language in French and English', async () => {
    for (const lng of ['fr', 'en'] as const) {
      const html = await renderForm(await i18nFor(lng));
      const select = html.match(/id="new-reader-language"[\s\S]*?<\/select>/)?.[0] ?? '';
      assert.match(select, /id="new-reader-language"/);
      assert.match(select, new RegExp(`<option value="" selected="">${LIBRARY_OPTION[lng]}</option>`));
      assert.match(select, /<option value="de">Deutsch<\/option>/);
      assert.equal(/<option value="fr" selected/.test(select), false);
    }
  });

  it('keeps the stored language selected when editing', async () => {
    const html = await renderForm(await i18nFor('en'), {
      mode: 'edit',
      user: { id: '1', username: 'ada', language: 'german' } as User,
    });
    assert.match(html, /<option value="de" selected="">Deutsch<\/option>/);
  });

  it('labels the empty option in German and Spanish', async () => {
    for (const lng of ['de', 'es'] as const) {
      const html = await renderForm(await i18nFor(lng));
      const select = html.match(/id="new-reader-language"[\s\S]*?<\/select>/)?.[0] ?? '';
      assert.match(select, new RegExp(`<option value="" selected="">${LIBRARY_OPTION[lng]}</option>`));
    }
  });
});
