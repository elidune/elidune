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

describe('reader card labels', () => {
  it('uses card-number wording for a reader and keeps barcode wording for items', () => {
    assert.equal(fr.profile.barcode, 'N° de carte');
    assert.equal(en.profile.barcode, 'Card number');
    assert.equal(de.profile.barcode, 'Leseausweis-Nr.');
    assert.equal(es.profile.barcode, 'N.º de carné');
    assert.equal(fr.users.searchPlaceholder, 'Rechercher par nom, prénom ou n° de carte...');
    assert.equal(en.users.searchPlaceholder, 'Search by name, first name or card number…');
    assert.equal(de.users.searchPlaceholder, 'Nach Name, Vorname oder Leseausweis-Nr. suchen...');
    assert.equal(es.users.searchPlaceholder, 'Buscar por nombre, apellido o n.º de carné...');
    assert.equal(fr.loans.userNotFound, 'Utilisateur non trouvé avec le n° de carte : {{barcode}}');
    assert.equal(en.loans.userNotFound, 'User not found with card number: {{barcode}}');
    assert.equal(de.loans.userNotFound, 'Benutzer mit Leseausweis-Nr. nicht gefunden: {{barcode}}');
    assert.equal(es.loans.userNotFound, 'Usuario no encontrado con el n.º de carné: {{barcode}}');
    assert.equal(fr.loans.scanOrEnterCard, 'Scanner ou saisir le n° de carte');
    assert.equal(en.loans.scanOrEnterCard, 'Scan or enter card number');
    assert.equal(de.loans.scanOrEnterCard, 'Leseausweis-Nr. scannen oder eingeben');
    assert.equal(es.loans.scanOrEnterCard, 'Escanear o introducir el n.º de carné');
    assert.equal(fr.loans.scanOrEnterBarcode, 'Scanner ou saisir le code-barres');
    assert.equal(en.loans.scanOrEnterBarcode, 'Scan or enter the barcode');
    assert.equal(fr.items.barcode, 'Code-barres');
    assert.equal(en.items.barcode, 'Barcode');
    assert.match(fr.users.deleteConfirm, /n° de carte/);
    assert.equal(fr.users.deleteConfirm.includes('code-barres'), false);
    assert.match(en.users.deleteConfirm, /card number/);
    assert.equal(en.users.deleteConfirm.includes('library card no.'), false);
    assert.equal(/\bbarcode\b/i.test(en.users.deleteConfirm), false);
    assert.match(es.users.deleteConfirm, /n\.º de carné/);
    assert.equal(es.users.deleteConfirm.includes('carné n.º'), false);
    assert.match(de.users.deleteConfirm, /Leseausweis-Nr\./);
    assert.equal(de.users.deleteConfirm.includes('Leseausweis-Nr.'), true);
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

  it('has an explicit administrator label in every locale', () => {
    // German copy uses masculine generics (Leser, Bibliothekar), not gendered forms.
    assert.equal(en.codes.accountType.admin, 'Administrator');
    assert.equal(fr.codes.accountType.admin, 'Administrateur');
    assert.equal(de.codes.accountType.admin, 'Administrator');
    assert.equal(es.codes.accountType.admin, 'Administrador');
    assert.equal(en.users.administrator, 'Administrator');
    assert.equal(fr.users.administrator, 'Administrateur');
    assert.equal(de.users.administrator, 'Administrator');
    assert.equal(es.users.administrator, 'Administrador');
  });

  it('translates the English seed name shown in the sidebar', () => {
    const administrator = [role('admin', 'Administrator')];
    assert.equal(localizedAccountTypeLabel(tFr, 'admin', administrator), 'Administrateur');
    assert.equal(localizedAccountTypeLabel(tEn, 'admin', administrator), 'Administrator');
    assert.equal(localizedAccountTypeLabel(tDe, 'admin', administrator), 'Administrator');
    assert.equal(localizedAccountTypeLabel(tEs, 'admin', administrator), 'Administrador');
    assert.equal(localizedAccountTypeLabel(tFr, 'Administrator', administrator), 'Administrateur');
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
