import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import { localizedAccountTypeLabel } from './accountTypeDisplay.ts';
import type { AccountTypeDefinition } from '../types/index.ts';

type Catalog = { codes: { accountType: Record<string, string> }; items: { barcode: string } };

function tFrom(catalog: Catalog): (key: string) => string {
  return (key: string) => {
    const [section, group, name] = key.split('.');
    if (section === 'codes' && group === 'accountType' && name && catalog.codes.accountType[name]) {
      return catalog.codes.accountType[name];
    }
    return key;
  };
}

function role(code: string, name: string | null): AccountTypeDefinition {
  return {
    code,
    name,
    itemsRights: null,
    usersRights: null,
    loansRights: null,
    itemsArchiveRights: null,
    holdsRights: null,
    settingsRights: null,
    eventsRights: null,
  };
}

describe('item barcode label', () => {
  it('translates the desk item-line label in every UI language', () => {
    assert.equal(en.items.barcode, 'Barcode');
    assert.equal(fr.items.barcode, 'Code-barres');
    assert.equal(de.items.barcode, 'Barcode');
    assert.equal(es.items.barcode, 'Código de barras');
    for (const catalog of [en, fr, de, es]) {
      assert.notEqual(catalog.items.barcode, 'barcode');
    }
  });
});

describe('account type labels', () => {
  const tFr = tFrom(fr);
  const tEn = tFrom(en);
  const tDe = tFrom(de);
  const tEs = tFrom(es);

  it('translates built-in role keys instead of showing the raw code', () => {
    assert.equal(localizedAccountTypeLabel(tFr, 'reader'), 'Lecteur');
    assert.equal(localizedAccountTypeLabel(tFr, 'librarian'), 'Bibliothécaire');
    assert.equal(localizedAccountTypeLabel(tFr, 'admin'), 'Administrateur');
    assert.equal(localizedAccountTypeLabel(tFr, 'administrator'), 'Administrateur');
    assert.equal(localizedAccountTypeLabel(tFr, 'guest'), 'Invité');
    assert.equal(localizedAccountTypeLabel(tFr, 'group'), 'Groupe');
    assert.equal(localizedAccountTypeLabel(tFr, '2'), 'Lecteur');
    assert.equal(localizedAccountTypeLabel(tEn, 'reader'), 'Reader');
    assert.equal(localizedAccountTypeLabel(tDe, 'reader'), 'Leser');
    assert.equal(localizedAccountTypeLabel(tEs, 'reader'), 'Lector');
  });

  it('keeps a customized catalog name and falls back for unknown codes', () => {
    assert.equal(
      localizedAccountTypeLabel(tFr, 'reader', [role('reader', 'Reader')]),
      'Lecteur',
    );
    assert.equal(
      localizedAccountTypeLabel(tFr, 'reader', [role('reader', 'Adhérent')]),
      'Adhérent',
    );
    assert.equal(
      localizedAccountTypeLabel(tFr, 'volunteer', [role('volunteer', 'Bénévole')]),
      'Bénévole',
    );
    assert.equal(localizedAccountTypeLabel(tFr, 'volunteer'), 'volunteer');
    assert.equal(localizedAccountTypeLabel(tFr, '  '), '—');
    assert.equal(localizedAccountTypeLabel(tFr, null), '—');
  });
});
