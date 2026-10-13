import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import i18next from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { formatCatalogLanguage } from './catalogLanguage.ts';

describe('catalog language labels', () => {
  const i18n = i18next.createInstance();

  it('maps API language names and ISO codes into the UI locale', async () => {
    await i18n.init({
      lng: 'fr',
      fallbackLng: 'fr',
      resources: {
        en: { translation: en },
        fr: { translation: fr },
        de: { translation: de },
        es: { translation: es },
      },
      interpolation: { escapeValue: false },
    });

    const unspecified = (lng: string) => i18n.t('codes.lang.unknown', { lng });

    assert.equal(formatCatalogLanguage('french', 'fr', unspecified('fr')), 'Français');
    assert.equal(formatCatalogLanguage('french', 'en', unspecified('en')), 'French');
    assert.equal(formatCatalogLanguage('french', 'de', unspecified('de')), 'Französisch');
    assert.equal(formatCatalogLanguage('french', 'es', unspecified('es')), 'Francés');
    assert.equal(formatCatalogLanguage('english', 'fr'), 'Anglais');
    assert.equal(formatCatalogLanguage('eng', 'fr'), 'Anglais');
    assert.equal(formatCatalogLanguage('fra', 'de'), 'Französisch');
    assert.equal(formatCatalogLanguage('fr', 'es'), 'Francés');
    assert.equal(formatCatalogLanguage('jp', 'en'), 'Japanese');
    assert.equal(formatCatalogLanguage('langFr', 'fr'), 'Français');
    assert.equal(formatCatalogLanguage('1', 'en'), 'French');
    assert.equal(formatCatalogLanguage('fr-FR', 'fr'), 'Français (France)');
    assert.equal(formatCatalogLanguage('unknown', 'fr', unspecified('fr')), 'non spécifié');
    assert.equal(formatCatalogLanguage('unknown', 'en', unspecified('en')), 'Not specified');
    assert.equal(formatCatalogLanguage('  Klingon  ', 'fr'), 'Klingon');
    assert.equal(formatCatalogLanguage('', 'fr'), '');
    assert.equal(formatCatalogLanguage(null, 'fr'), '');
  });
});
