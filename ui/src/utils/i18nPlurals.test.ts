import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import i18next from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };

const catalogs = { en, fr, de, es };

const migratedPaths = [
  ['items', 'batchDeleteScan', 'entries'],
  ['items', 'batchDeleteScan', 'deletedCount'],
  ['z3950', 'documentsFound'],
  ['importMarc', 'validationErrorCount'],
  ['importMarc', 'batchRecords'],
  ['importMarc', 'documentsCount'],
  ['importMarc', 'errors'],
  ['importMarc', 'duplicateExistingCount'],
  ['importMarc', 'remainingToScan'],
  ['loans', 'count'],
] as const;

function walkKeys(node: unknown, visit: (key: string) => void): void {
  if (!node || typeof node !== 'object' || Array.isArray(node)) return;
  for (const [key, value] of Object.entries(node)) {
    visit(key);
    walkKeys(value, visit);
  }
}

function parentOf(catalog: object, path: readonly string[]): Record<string, unknown> {
  let node: unknown = catalog;
  for (const segment of path.slice(0, -1)) {
    assert.ok(node && typeof node === 'object');
    node = (node as Record<string, unknown>)[segment];
  }
  assert.ok(node && typeof node === 'object' && !Array.isArray(node));
  return node as Record<string, unknown>;
}

describe('i18n plural keys', () => {
  it('has no leftover v3 _plural keys', () => {
    for (const catalog of Object.values(catalogs)) {
      walkKeys(catalog, (key) => {
        assert.equal(key.endsWith('_plural'), false, key);
      });
    }
  });

  it('stores migrated counts as i18next one/other forms', () => {
    for (const catalog of Object.values(catalogs)) {
      for (const path of migratedPaths) {
        const parent = parentOf(catalog, path);
        const key = path[path.length - 1];
        assert.equal(key in parent, false, path.join('.'));
        assert.equal(`${key}_plural` in parent, false, path.join('.'));
        assert.equal(typeof parent[`${key}_one`], 'string', path.join('.'));
        assert.equal(typeof parent[`${key}_other`], 'string', path.join('.'));
      }
    }
  });

  it('resolves loan, document, and scan counts', async () => {
    const i18n = i18next.createInstance();
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

    assert.equal(i18n.t('loans.count', { lng: 'fr', count: 0 }), '0 emprunt');
    assert.equal(i18n.t('loans.count', { lng: 'fr', count: 1 }), '1 emprunt');
    assert.equal(i18n.t('loans.count', { lng: 'fr', count: 2 }), '2 emprunts');
    assert.equal(i18n.t('loans.count', { lng: 'en', count: 1 }), '1 loan');
    assert.equal(i18n.t('loans.count', { lng: 'en', count: 2 }), '2 loans');
    assert.equal(i18n.t('loans.count', { lng: 'de', count: 1 }), '1 Ausleihe');
    assert.equal(i18n.t('loans.count', { lng: 'de', count: 2 }), '2 Ausleihen');
    assert.equal(i18n.t('loans.count', { lng: 'es', count: 1 }), '1 préstamo');
    assert.equal(i18n.t('loans.count', { lng: 'es', count: 2 }), '2 préstamos');

    assert.equal(i18n.t('z3950.documentsFound', { lng: 'fr', count: 1 }), '1 document trouvé');
    assert.equal(i18n.t('z3950.documentsFound', { lng: 'fr', count: 2 }), '2 documents trouvés');
    assert.equal(i18n.t('importMarc.documentsCount', { lng: 'fr', count: 2 }), '2 documents');
    assert.equal(i18n.t('importMarc.batchRecords', { lng: 'fr', count: 1 }), '1 notice');
    assert.equal(i18n.t('importMarc.batchRecords', { lng: 'fr', count: 4 }), '4 notices');
    assert.equal(i18n.t('importMarc.errors', { lng: 'en', count: 1 }), '1 error');
    assert.equal(i18n.t('importMarc.errors', { lng: 'en', count: 3 }), '3 errors');
    assert.equal(i18n.t('importMarc.remainingToScan', { lng: 'de', count: 2 }), '2 Dokumente noch zu scannen');
    assert.equal(i18n.t('importMarc.duplicateExistingCount', { lng: 'es', count: 2 }), '2 registros existentes');
    assert.equal(i18n.t('items.batchDeleteScan.entries', { lng: 'fr', count: 2 }), '2 entrées');
    assert.equal(i18n.t('items.batchDeleteScan.deletedCount', { lng: 'fr', count: 1 }), '1 supprimé');
    assert.equal(i18n.t('items.batchDeleteScan.deletedCount', { lng: 'de', count: 2 }), '2 gelöscht');
  });
});
