import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import i18next from 'i18next';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { PublicType } from '../types/index.ts';
import { localizedPublicTypeById, localizedPublicTypeLabel } from './publicTypeDisplay.ts';

const catalogs = { en, fr, de, es };

function patronType(name: string, label: string, id = '1'): PublicType {
  return {
    id,
    name,
    label,
    unpaidThresholdMode: 'inherit',
  };
}

describe('patron type labels', () => {
  it('translates the English seed captions in every locale', () => {
    assert.deepEqual(en.codes.patronType, {
      child: 'Child',
      adult: 'Adult',
      school: 'School',
      staff: 'Staff',
      senior: 'Senior',
    });
    assert.deepEqual(fr.codes.patronType, {
      child: 'Enfant',
      adult: 'Adulte',
      school: 'Établissement scolaire',
      staff: 'Personnel',
      senior: 'Senior',
    });
    assert.deepEqual(de.codes.patronType, {
      child: 'Kind',
      adult: 'Erwachsener',
      school: 'Schule',
      staff: 'Personal',
      senior: 'Senior',
    });
    assert.deepEqual(es.codes.patronType, {
      child: 'Infantil',
      adult: 'Adulto',
      school: 'Centro escolar',
      staff: 'Personal',
      senior: 'Sénior',
    });

    for (const catalog of Object.values(catalogs)) {
      const t = (key: string) => {
        const name = key.split('.')[2];
        return catalog.codes.patronType[name as keyof typeof catalog.codes.patronType] ?? key;
      };
      for (const [name, english] of [
        ['child', 'Child'],
        ['adult', 'Adult'],
        ['school', 'School'],
        ['staff', 'Staff'],
        ['senior', 'Senior'],
      ] as const) {
        assert.equal(localizedPublicTypeLabel(t, patronType(name, english)), catalog.codes.patronType[name]);
      }
    }
  });

  it('keeps a library caption that is no longer the English seed', () => {
    const t = (key: string) => fr.codes.patronType[key.split('.')[2] as keyof typeof fr.codes.patronType] ?? key;
    assert.equal(localizedPublicTypeLabel(t, patronType('adult', 'Adulte tarif réduit')), 'Adulte tarif réduit');
    assert.equal(localizedPublicTypeLabel(t, patronType('collectivite', 'Collectivité')), 'Collectivité');
  });

  it('resolves a patron record id to the translated caption', () => {
    const t = (key: string) => fr.codes.patronType[key.split('.')[2] as keyof typeof fr.codes.patronType] ?? key;
    const types = [patronType('child', 'Child', '7'), patronType('adult', 'Adult', '8')];
    assert.equal(localizedPublicTypeById(t, types, 7), 'Enfant');
    assert.equal(localizedPublicTypeById(t, types, '8'), 'Adulte');
    assert.equal(localizedPublicTypeById(t, types, null), null);
    assert.equal(localizedPublicTypeById(t, types, '99'), null);
  });
});

describe('patron list count', () => {
  it('stores the header as i18next one/other forms', () => {
    for (const catalog of Object.values(catalogs)) {
      const users = catalog.users as Record<string, unknown>;
      assert.equal('count' in users, false);
      assert.equal('count_plural' in users, false);
      assert.equal(typeof users.count_one, 'string');
      assert.equal(typeof users.count_other, 'string');
    }
  });

  it('uses the plural for more than one patron', async () => {
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

    assert.equal(i18n.t('users.count', { lng: 'fr', count: 1 }), '1 usager');
    assert.equal(i18n.t('users.count', { lng: 'fr', count: 3 }), '3 usagers');
    assert.equal(i18n.t('users.count', { lng: 'fr', count: 121 }), '121 usagers');
    assert.equal(i18n.t('users.count', { lng: 'en', count: 1 }), '1 user');
    assert.equal(i18n.t('users.count', { lng: 'en', count: 3 }), '3 users');
    assert.equal(i18n.t('users.count', { lng: 'de', count: 1 }), '1 Benutzer');
    assert.equal(i18n.t('users.count', { lng: 'de', count: 3 }), '3 Benutzer');
    assert.equal(i18n.t('users.count', { lng: 'es', count: 1 }), '1 usuario');
    assert.equal(i18n.t('users.count', { lng: 'es', count: 3 }), '3 usuarios');
  });
});
