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
  isLibraryLanguageValidationError,
  libraryLanguageChoice,
  libraryLanguageForUpdate,
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

  it('uses the library language on the page when the browser language is unsupported, and French when it is null', async () => {
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

    const fromLibrary = resolveUiLanguage({
      browserLanguage: 'it-IT',
      libraryLanguage: libraryLanguageFromSettings({ language: 'de', timezone: 'Europe/Berlin' }),
    });
    assert.equal(fromLibrary, 'de');
    await applyUiLanguage(i18n, fromLibrary, root);
    assert.equal(i18n.resolvedLanguage ?? i18n.language, 'de');
    assert.equal(root.lang, 'de');
    assert.equal(i18n.t('language.selector'), 'Sprache');
    assert.equal(i18n.t('library.general.saveSuccess'), 'Einstellungen gespeichert');

    const fromFallback = resolveUiLanguage({
      browserLanguage: 'it-IT',
      libraryLanguage: libraryLanguageFromSettings({ language: null, timezone: 'Europe/Paris' }),
    });
    assert.equal(fromFallback, 'fr');
    await applyUiLanguage(i18n, fromFallback, root);
    assert.equal(i18n.resolvedLanguage ?? i18n.language, 'fr');
    assert.equal(root.lang, 'fr');
    assert.equal(i18n.t('language.selector'), 'Langue');
    assert.equal(i18n.t('library.general.saveSuccess'), 'Réglages enregistrés');
    assert.notEqual(fromFallback, 'en');
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

describe('library default language setting', () => {
  it('sends null for Non définie and a UI code for Français, English, Deutsch, and Español', () => {
    assert.equal(libraryLanguageChoice(null), '');
    assert.equal(libraryLanguageChoice(undefined), '');
    assert.equal(libraryLanguageChoice(''), '');
    assert.equal(libraryLanguageChoice('it'), '');
    assert.equal(libraryLanguageForUpdate(''), null);
    assert.equal(libraryLanguageForUpdate(null), null);

    for (const code of ['fr', 'en', 'de', 'es'] as const) {
      assert.equal(libraryLanguageChoice(code), code);
      assert.equal(libraryLanguageForUpdate(code), code);
    }
  });

  it('treats a 400 validation_error about language as an inline field error', () => {
    assert.equal(
      isLibraryLanguageValidationError({
        status: 400,
        code: 'validation_error',
        message: 'language must be one of: fr, en, de, es',
      }),
      true,
    );
    assert.equal(
      isLibraryLanguageValidationError({
        status: 400,
        code: 'validation_error',
        message: "Unrecognized country 'ZZ'",
      }),
      false,
    );
    assert.equal(
      isLibraryLanguageValidationError({
        status: 400,
        code: 'invalid_timezone',
        message: 'language must be one of: fr, en, de, es',
      }),
      false,
    );
    assert.equal(
      isLibraryLanguageValidationError({
        status: 500,
        code: 'validation_error',
        message: 'language must be one of: fr, en, de, es',
      }),
      false,
    );
  });

  it('translates the library default language label, unset option, and help text', () => {
    const labels = {
      fr: {
        defaultLanguage: 'Langue par défaut de la bibliothèque',
        defaultLanguageUnset: 'Non définie',
      },
      en: {
        defaultLanguage: 'Library default language',
        defaultLanguageUnset: 'Not set',
      },
      de: {
        defaultLanguage: 'Standardsprache der Bibliothek',
        defaultLanguageUnset: 'Nicht festgelegt',
      },
      es: {
        defaultLanguage: 'Idioma predeterminado de la biblioteca',
        defaultLanguageUnset: 'Sin definir',
      },
    } as const;
    const catalogs = { fr, en, de, es };
    for (const code of ['fr', 'en', 'de', 'es'] as const) {
      const general = catalogs[code].library.general;
      assert.equal(general.defaultLanguage, labels[code].defaultLanguage);
      assert.equal(general.defaultLanguageUnset, labels[code].defaultLanguageUnset);
      assert.equal(typeof general.defaultLanguageHelp, 'string');
      assert.ok(general.defaultLanguageHelp.length > 0);
      assert.equal(typeof general.defaultLanguageInvalid, 'string');
      assert.ok(general.defaultLanguageInvalid.length > 0);
    }
  });
});
