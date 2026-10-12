import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createInstance } from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { PUBLIC_LANGUAGE_STORAGE_KEY } from '../locales/languages.ts';
import {
  applyUiLanguage,
  libraryLanguageFromSettings,
  resolveUiLanguage,
} from './uiLanguage.ts';

const SELECTOR_LABEL = {
  fr: 'Langue',
  en: 'Language',
  de: 'Sprache',
  es: 'Idioma',
} as const;

describe('public language choice applies without a reload', () => {
  it('switches the selector label, catalogue tab, and html lang for fr, de, and es', async () => {
    const i18n = createInstance();
    await i18n.init({
      resources: {
        en: { translation: en },
        fr: { translation: fr },
        de: { translation: de },
        es: { translation: es },
      },
      lng: 'en',
      fallbackLng: 'fr',
      supportedLngs: ['en', 'fr', 'de', 'es'],
      interpolation: { escapeValue: false },
    });
    const root = { lang: 'en' };
    assert.equal(i18n.t('language.selector'), 'Language');
    assert.equal(i18n.t('opac.tabCatalog'), 'Catalogue');

    for (const code of ['fr', 'de', 'es'] as const) {
      const next = resolveUiLanguage({
        explicitChoice: code,
        browserLanguage: 'en-US',
        libraryLanguage: 'fr',
      });
      await applyUiLanguage(i18n, next, root);
      assert.equal(i18n.resolvedLanguage ?? i18n.language, code);
      assert.equal(root.lang, code);
      assert.equal(i18n.t('language.selector'), SELECTOR_LABEL[code]);
    }

    assert.equal(de.opac.tabCatalog, 'Katalog');
    assert.equal(es.opac.tabCatalog, 'Catálogo');
    assert.notEqual(de.opac.tabCatalog, 'Catalogue');
    assert.notEqual(es.opac.tabCatalog, 'Catalogue');
    assert.equal(i18n.t('opac.tabCatalog'), 'Catálogo');
  });
});

describe('UI language resolution', () => {
  it('prefers the account preference over an explicit choice, the browser, and the library', () => {
    assert.equal(
      resolveUiLanguage({
        accountLanguage: 'french',
        explicitChoice: 'en',
        browserLanguage: 'de-DE',
        libraryLanguage: 'spanish',
      }),
      'fr',
    );
    assert.equal(
      resolveUiLanguage({
        accountLanguage: 'en',
        explicitChoice: 'es',
        browserLanguage: 'it',
        libraryLanguage: 'de',
      }),
      'en',
    );
  });

  it('uses an explicit public choice before a supported browser language and the library', () => {
    assert.equal(
      resolveUiLanguage({
        explicitChoice: 'es',
        browserLanguage: 'en-US',
        libraryLanguage: 'german',
      }),
      'es',
    );
    assert.equal(
      resolveUiLanguage({
        explicitChoice: 'de',
        browserLanguage: 'fr',
        libraryLanguage: 'en',
      }),
      'de',
    );
  });

  it('uses a supported browser language before the library language', () => {
    assert.equal(
      resolveUiLanguage({
        browserLanguage: 'de-DE',
        libraryLanguage: 'french',
      }),
      'de',
    );
    assert.equal(
      resolveUiLanguage({
        browserLanguage: 'en_GB',
        libraryLanguage: 'es',
      }),
      'en',
    );
  });

  it('uses the library language when the browser language is unsupported', () => {
    assert.equal(
      resolveUiLanguage({
        browserLanguage: 'it',
        libraryLanguage: 'german',
      }),
      'de',
    );
    assert.equal(
      resolveUiLanguage({
        browserLanguage: 'it-IT',
        libraryLanguage: libraryLanguageFromSettings({ language: 'es' }),
      }),
      'es',
    );
  });

  it('uses library-info language (fr, en, de, es) when the browser language is unsupported', () => {
    for (const code of ['fr', 'en', 'de', 'es'] as const) {
      const payload = { name: 'Stadtbibliothek', timezone: 'Europe/Berlin', language: code };
      assert.equal(libraryLanguageFromSettings(payload), code);
      assert.equal(
        resolveUiLanguage({
          browserLanguage: 'it',
          libraryLanguage: libraryLanguageFromSettings(payload),
        }),
        code,
      );
    }
  });

  it('falls back to french only when library-info language is absent or empty', () => {
    assert.equal(libraryLanguageFromSettings({ name: 'Médiathèque', timezone: 'Europe/Paris' }), null);
    assert.equal(libraryLanguageFromSettings({ language: '' }), null);
    assert.equal(libraryLanguageFromSettings({ language: '   ' }), null);
    assert.equal(libraryLanguageFromSettings({ language: null }), null);
    assert.equal(libraryLanguageFromSettings(null), null);
    assert.equal(
      libraryLanguageFromSettings({ language: '', defaultLocale: 'en' }),
      null,
    );
    for (const libraryLanguage of [
      libraryLanguageFromSettings({ name: 'Médiathèque' }),
      libraryLanguageFromSettings({ language: '' }),
      libraryLanguageFromSettings({ language: '   ' }),
    ]) {
      assert.equal(resolveUiLanguage({ browserLanguage: 'it', libraryLanguage }), 'fr');
    }
  });

  it('falls back to french when the browser language is unsupported and the library has no language', () => {
    const publicLibraryInfo = {
      name: 'Médiathèque',
      timezone: 'Europe/Paris',
      phones: [],
    };
    assert.equal(libraryLanguageFromSettings(publicLibraryInfo), null);
    assert.equal(libraryLanguageFromSettings(null), null);
    assert.equal(
      resolveUiLanguage({
        browserLanguage: 'it',
        libraryLanguage: libraryLanguageFromSettings(publicLibraryInfo),
      }),
      'fr',
    );
    assert.equal(resolveUiLanguage({ browserLanguage: 'it' }), 'fr');
    assert.equal(resolveUiLanguage({ browserLanguage: 'it', libraryLanguage: 'italian' }), 'fr');
    assert.notEqual(resolveUiLanguage({ browserLanguage: 'it' }), 'en');
  });

  it('ignores an unsupported explicit choice and keeps a stored public choice across a reload', () => {
    assert.equal(
      resolveUiLanguage({
        explicitChoice: 'it',
        browserLanguage: 'fr-FR',
        libraryLanguage: 'de',
      }),
      'fr',
    );

    const stored = new Map<string, string>();
    const storage = {
      getItem: (key: string) => stored.get(key) ?? null,
      setItem: (key: string, value: string) => {
        stored.set(key, value);
      },
    };
    storage.setItem(PUBLIC_LANGUAGE_STORAGE_KEY, 'es');
    const explicitChoice = storage.getItem(PUBLIC_LANGUAGE_STORAGE_KEY);
    assert.equal(
      resolveUiLanguage({
        explicitChoice,
        browserLanguage: 'it',
        libraryLanguage: 'fr',
      }),
      'es',
    );
    assert.equal(
      resolveUiLanguage({
        accountLanguage: 'english',
        explicitChoice,
        browserLanguage: 'it',
        libraryLanguage: 'de',
      }),
      'en',
    );
  });
});
