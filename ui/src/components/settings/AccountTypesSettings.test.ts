import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { createInstance, type i18n as I18nInstance, type TFunction } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import de from '../../locales/de/translation.json' with { type: 'json' };
import en from '../../locales/en/translation.json' with { type: 'json' };
import es from '../../locales/es/translation.json' with { type: 'json' };
import fr from '../../locales/fr/translation.json' with { type: 'json' };
import AccountTypeSelect from '@/components/users/AccountTypeSelect';
import { AccountTypesEditor } from '@/components/settings/AccountTypesEditor';
import type { AccountTypeDefinition } from '@/types';
import { canWriteSettings } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import {
  ACCOUNT_TYPE_RIGHT_FIELDS,
  accountTypeCodeError,
  blankAccountTypeRights,
  buildCreateAccountTypeRequest,
  isBuiltInAccountType,
} from '@/utils/accountTypeEditor';

type Lang = 'fr' | 'en' | 'de' | 'es';

const catalogs = { fr, en, de, es };

function httpError(status: number, code: string, message: string) {
  return {
    isAxiosError: true,
    message: 'Request failed',
    response: { status, data: { code, error: code, message } },
  };
}

function tokenWith(rights: Record<string, string>): string {
  const payload = Buffer.from(JSON.stringify({ rights })).toString('base64url');
  return `header.${payload}.sig`;
}

async function i18nFor(lng: Lang): Promise<I18nInstance> {
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

function render(i18n: I18nInstance, node: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nextProvider, { i18n }, node) as ReactElement);
}

function role(
  code: string,
  name: string | null,
  isSystem?: boolean,
): AccountTypeDefinition {
  return {
    code,
    name,
    itemsRights: 'n',
    usersRights: 'n',
    loansRights: 'n',
    itemsArchiveRights: 'n',
    holdsRights: 'n',
    settingsRights: 'n',
    eventsRights: 'n',
    isSystem,
  };
}

const noop = () => {};

describe('account type error codes', () => {
  const codes = ['account_type_in_use', 'account_type_system', 'last_active_admin'] as const;

  for (const lng of ['fr', 'en', 'de', 'es'] as const) {
    it(`maps the three 409 codes in ${lng}`, async () => {
      const i18n = await i18nFor(lng);
      const t = i18n.t.bind(i18n) as TFunction;
      const catalog = catalogs[lng].errors.apiCode;
      for (const code of codes) {
        const serverText = `raw ${code}`;
        const shown = getApiErrorMessage(httpError(409, code, serverText), t);
        assert.equal(shown, catalog[code]);
        assert.equal(shown.includes(serverText), false);
      }
      assert.match(catalogs.fr.errors.apiCode.account_type_in_use, /lecteur/);
      assert.equal(catalogs.fr.settings.accountTypes.rights.itemsRights, 'Documents');
      assert.equal(catalogs.fr.settings.accountTypes.rights.usersRights, 'Lecteurs');
      assert.equal(
        catalogs.fr.settings.accountTypes.rights.circulationOverrideRights,
        'Forcer les dérogations au comptoir (exemplaire réservé pour un autre lecteur, abonnement expiré)',
      );
      assert.equal(
        catalogs.en.settings.accountTypes.rights.circulationOverrideRights,
        'Force desk exceptions (copy reserved for another reader, expired subscription)',
      );
      assert.equal(
        catalogs.fr.settings.accountTypes.rights.circulationOverrideRights.toLowerCase().includes('spécimen'),
        false,
      );
    });
  }
});

describe('account type settings screen', () => {
  it('marks built-in types and offers delete only for custom types', async () => {
    const i18n = await i18nFor('fr');
    const rows = [role('reader', 'Reader', true), role('loan_agent', 'Agent de prêt', false)];
    const html = render(
      i18n,
      createElement(AccountTypesEditor, {
        rows,
        originals: rows,
        canEdit: true,
        savingCode: null,
        rowError: null,
        banner: null,
        createOpen: true,
        createCode: '',
        createName: '',
        createRights: blankAccountTypeRights(),
        createFieldError: null,
        createError: null,
        creating: false,
        onUpdateRow: noop,
        onSaveRow: noop,
        onDeleteRow: noop,
        onOpenCreate: noop,
        onCancelCreate: noop,
        onCreateCode: noop,
        onCreateName: noop,
        onCreateRight: noop,
        onSubmitCreate: noop,
      }),
    );

    assert.match(html, /Intégré/);
    assert.match(html, /Documents/);
    assert.match(html, /Lecteurs/);
    assert.match(html, /Forcer les dérogations au comptoir \(exemplaire réservé pour un autre lecteur, abonnement expiré\)/);
    assert.equal(html.toLowerCase().includes('spécimen'), false);
    assert.match(html, /aria-label="Supprimer Agent de prêt"/);
    assert.equal(html.includes('Supprimer Reader'), false);
    assert.equal(html.includes('Supprimer Lecteur'), false);
    assert.match(html, /id="account-type-create-code"/);
    assert.equal(isBuiltInAccountType(role('admin', 'Administrator', true)), true);
    assert.equal(isBuiltInAccountType(role('loan_agent', 'Agent de prêt', false)), false);
    assert.equal(isBuiltInAccountType(role('reader', 'Reader')), false);
  });

  it('hides create and delete without settings write', async () => {
    const i18n = await i18nFor('en');
    const rows = [role('loan_agent', 'Loan agent', false)];
    const html = render(
      i18n,
      createElement(AccountTypesEditor, {
        rows,
        originals: rows,
        canEdit: false,
        savingCode: null,
        rowError: null,
        banner: null,
        createOpen: false,
        createCode: '',
        createName: '',
        createRights: blankAccountTypeRights(),
        createFieldError: null,
        createError: null,
        creating: false,
        onUpdateRow: noop,
        onSaveRow: noop,
        onDeleteRow: noop,
        onOpenCreate: noop,
        onCancelCreate: noop,
        onCreateCode: noop,
        onCreateName: noop,
        onCreateRight: noop,
        onSubmitCreate: noop,
      }),
    );
    assert.equal(html.includes('Delete Loan agent'), false);
    assert.equal(html.includes('New account type'), false);
    assert.match(html, /settings write/);
  });
});

describe('user form account type picker', () => {
  it('lists only the account types returned by the API, including custom codes', async () => {
    const i18n = await i18nFor('fr');
    const html = render(
      i18n,
      createElement(AccountTypeSelect, {
        id: 'account-type',
        value: 'reader',
        accountTypes: [role('reader', 'Reader', true), role('loan_agent', 'Agent de prêt', false)],
        onChange: noop,
      }),
    );
    assert.match(html, /value="reader"/);
    assert.match(html, /Lecteur/);
    assert.match(html, /value="loan_agent"/);
    assert.match(html, /Agent de prêt/);
    assert.equal(html.includes('value="librarian"'), false);
    assert.equal(html.includes('value="admin"'), false);
  });
});

describe('account type create payload and settings write', () => {
  it('accepts a custom code and rejects a hyphen or a leading digit', () => {
    assert.equal(accountTypeCodeError('Loan_Agent'), null);
    assert.equal(accountTypeCodeError('agent-pret'), 'invalid');
    assert.equal(accountTypeCodeError('1agent'), 'invalid');
    assert.equal(accountTypeCodeError(''), 'required');
    const rights = blankAccountTypeRights();
    rights.loansRights = 'w';
    rights.circulationOverrideRights = 'w';
    const body = buildCreateAccountTypeRequest('Loan_Agent', ' Agent de prêt ', rights);
    assert.equal(body?.code, 'loan_agent');
    assert.equal(body?.name, 'Agent de prêt');
    assert.equal(body?.loansRights, 'w');
    assert.equal(body?.itemsRights, 'n');
    assert.equal(body?.circulationOverrideRights, 'w');
    assert.equal(buildCreateAccountTypeRequest('bad code', 'Label', rights), null);
    assert.equal(ACCOUNT_TYPE_RIGHT_FIELDS.includes('circulationOverrideRights'), true);
    assert.equal(Object.keys(blankAccountTypeRights()).length, ACCOUNT_TYPE_RIGHT_FIELDS.length);
    const rust = readFileSync(
      fileURLToPath(new URL('../../../../server/src/models/account_type.rs', import.meta.url)),
      'utf8',
    );
    const struct = rust.match(/pub struct AccountTypeDefinition \{([\s\S]*?)\n\}/);
    assert.ok(struct);
    const fromServer = [...struct[1].matchAll(/pub (\w+_rights):/g)].map((match) =>
      match[1].replace(/_([a-z])/g, (_all, letter: string) => letter.toUpperCase()),
    );
    assert.deepEqual([...ACCOUNT_TYPE_RIGHT_FIELDS].sort(), [...fromServer].sort());
  });

  it('reads settings write from the token and denies a missing claim', () => {
    assert.equal(canWriteSettings(null, tokenWith({ settingsRights: 'w' })), true);
    assert.equal(canWriteSettings(null, tokenWith({ settingsRights: 'r' })), false);
    assert.equal(canWriteSettings(null, tokenWith({ settingsRights: 'n' })), false);
    assert.equal(canWriteSettings({ settingsRights: 'w' }, null), true);
    assert.equal(canWriteSettings(null, null), false);
  });
});
