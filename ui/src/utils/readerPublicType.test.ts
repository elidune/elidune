import './testLocalStorage.ts';
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createElement, type ReactElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createInstance, type i18n as I18nInstance } from 'i18next';
import { I18nextProvider } from 'react-i18next';
import { MemoryRouter } from 'react-router-dom';
import de from '../locales/de/translation.json' with { type: 'json' };
import en from '../locales/en/translation.json' with { type: 'json' };
import es from '../locales/es/translation.json' with { type: 'json' };
import fr from '../locales/fr/translation.json' with { type: 'json' };
import type { UserFormData } from '../components/users/UserEditorForm.tsx';
import type { AccountTypeDefinition, PublicType, User } from '../types/index.ts';
import type { AuthContextType } from '../contexts/AuthContext.tsx';
import type { LanguageContextType } from '../contexts/LanguageContext.tsx';
import { getApiErrorMessage } from './apiError.ts';
import {
  completedAgeYears,
  guardianPublicTypeId,
  readerBirthDateFieldError,
  readerBirthDateRequired,
  resolvePublicTypeIdForAge,
} from './readerPublicType.ts';

const FR_REQUIRED = 'La date de naissance est obligatoire pour déterminer le type de public du lecteur.';
const EN_REQUIRED = "Birth date is required to determine the reader's public type.";
const FR_NO_BAND =
  'Aucun type de public ne correspond à cet âge. Un administrateur doit compléter les tranches d\'âge dans Paramètres > Types de public.';
const EN_NO_BAND =
  'No public type matches this age. An administrator must complete the age ranges in Settings > Public Types.';
const FR_MISSING = 'Date de naissance à compléter';
const FR_HINT = 'Le type de public est déterminé à partir de la date de naissance.';

const TODAY = new Date(Date.UTC(2026, 5, 15));

const BANDS = [
  { id: '1', name: 'child', ageMin: 0, ageMax: 12 },
  { id: '8', name: 'teen', ageMin: 0, ageMax: 17 },
  { id: '3', name: 'adult', ageMin: 18, ageMax: 99 },
  { id: '4', name: 'school' },
] as const;

function account(code: string): AccountTypeDefinition {
  return {
    code,
    name: code,
    itemsRights: null,
    usersRights: null,
    loansRights: null,
    itemsArchiveRights: null,
    holdsRights: null,
    settingsRights: null,
    eventsRights: null,
  };
}

function publicTypes(): PublicType[] {
  return [
    { id: '1', name: 'child', label: 'Child', ageMin: 0, ageMax: 12, unpaidThresholdMode: 'inherit' },
    { id: '3', name: 'adult', label: 'Adult', ageMin: 18, ageMax: 99, unpaidThresholdMode: 'inherit' },
  ];
}

function formData(overrides: Partial<UserFormData> = {}): UserFormData {
  return {
    login: 'ada',
    password: '',
    firstname: 'Ada',
    lastname: 'Lovelace',
    sex: 'f',
    email: '',
    phone: '',
    barcode: '',
    birthdate: '2010-06-15',
    addrStreet: '',
    addrZipCode: '',
    addrCity: 'Annecy',
    notes: '',
    fee: '',
    publicType: '3',
    accountType: 'reader',
    expiryUnlimited: true,
    expiryAt: '',
    guardianId: '',
    eventsConsent: false,
    language: '',
    ...overrides,
  };
}

function httpError(code: string, message: string) {
  return {
    isAxiosError: true,
    message: 'Request failed with status code 422',
    response: {
      status: 422,
      data: { code, message },
    },
  };
}

async function i18nFor(lng: 'fr' | 'en' | 'de' | 'es'): Promise<I18nInstance> {
  const i18n = createInstance();
  await i18n.init({
    lng,
    fallbackLng: 'en',
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

function inputById(html: string, id: string): string {
  const tags = html.match(/<input\b[^>]*>/g) ?? [];
  return tags.find((tag) => tag.includes(`id="${id}"`)) ?? '';
}

async function renderEditor(
  i18n: I18nInstance,
  props: Record<string, unknown>,
): Promise<string> {
  const { default: UserEditorForm } = await import('../components/users/UserEditorForm.tsx');
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, enabled: false } },
  });
  return renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client },
      createElement(
        I18nextProvider,
        { i18n },
        createElement(UserEditorForm, {
          mode: 'create',
          formId: 'reader-form',
          publicTypes: publicTypes(),
          accountTypes: [account('reader'), account('librarian')],
          onLoadingChange: () => undefined,
          onSuccess: () => undefined,
          ...props,
        }),
      ),
    ) as ReactElement,
  );
}

function authValue(user: User): AuthContextType {
  return {
    user,
    isLoading: false,
    isAuthenticated: true,
    mustChangePassword: false,
    pending2FA: null,
    login: async () => ({ requires2FA: false }),
    verify2FA: async () => ({}),
    verifyRecovery: async () => ({}),
    cancel2FA: () => undefined,
    logout: () => undefined,
    refreshProfile: async () => undefined,
    profileWarning: null,
    dismissProfileWarning: () => undefined,
    completeFirstSetup: async () => {},
  };
}

function languageValue(i18n: I18nInstance): LanguageContextType {
  return {
    language: i18n.language as LanguageContextType['language'],
    setLanguage: async () => undefined,
    availableLanguages: ['en', 'fr', 'de', 'es'],
    languageNames: { en: 'English', fr: 'Français', de: 'Deutsch', es: 'Español' },
    languageFlags: { en: '🇬🇧', fr: '🇫🇷', de: '🇩🇪', es: '🇪🇸' },
  };
}

async function renderProfile(i18n: I18nInstance, user: User): Promise<string> {
  const { AuthContext } = await import('../contexts/AuthContext.tsx');
  const { LanguageContext } = await import('../contexts/LanguageContext.tsx');
  const { LANGUAGE_FLAGS, LANGUAGE_NAMES, SUPPORTED_LANGUAGES } = await import('../locales/index.ts');
  const { ToastProvider } = await import('../contexts/ToastContext.tsx');
  const { default: ProfilePage } = await import('../pages/ProfilePage.tsx');
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, enabled: false } },
  });
  const language: LanguageContextType = {
    ...languageValue(i18n),
    availableLanguages: SUPPORTED_LANGUAGES,
    languageNames: LANGUAGE_NAMES,
    languageFlags: LANGUAGE_FLAGS,
  };
  return renderToStaticMarkup(
    createElement(
      QueryClientProvider,
      { client },
      createElement(
        AuthContext.Provider,
        { value: authValue(user) },
        createElement(
          LanguageContext.Provider,
          { value: language },
          createElement(
            ToastProvider,
            null,
            createElement(
              I18nextProvider,
              { i18n },
              createElement(MemoryRouter, null, createElement(ProfilePage)),
            ),
          ),
        ),
      ),
    ) as ReactElement,
  );
}

describe('reader public type from birth date', () => {
  it('picks the narrowest band, then the lowest id, and ignores unbounded types', () => {
    assert.equal(resolvePublicTypeIdForAge(BANDS, 10), '1');
    assert.equal(resolvePublicTypeIdForAge(BANDS, 15), '8');
    assert.equal(resolvePublicTypeIdForAge(BANDS, 30), '3');
    assert.equal(resolvePublicTypeIdForAge(BANDS, 13), '8');
    assert.equal(resolvePublicTypeIdForAge([{ id: '4', name: 'school' }], 40), null);
    assert.equal(
      resolvePublicTypeIdForAge(
        [
          { id: '8', name: 'b', ageMin: 18, ageMax: 30 },
          { id: '3', name: 'a', ageMin: 18, ageMax: 30 },
        ],
        20,
      ),
      '3',
    );
  });

  it('counts the anniversary as the new age', () => {
    const before = new Date(Date.UTC(2025, 5, 14));
    const on = new Date(Date.UTC(2025, 5, 15));
    assert.equal(completedAgeYears('2010-06-15', before), 14);
    assert.equal(completedAgeYears('2010-06-15', on), 15);
  });

  it('uses the age band only to decide a child guardian', () => {
    assert.equal(
      guardianPublicTypeId({
        accountType: 'reader',
        birthdate: '2014-06-15',
        storedPublicTypeId: '3',
        publicTypes: BANDS,
        today: TODAY,
      }),
      '1',
    );
    assert.equal(
      guardianPublicTypeId({
        accountType: 'reader',
        birthdate: '',
        storedPublicTypeId: '3',
        publicTypes: BANDS,
        today: TODAY,
      }),
      '3',
    );
    assert.equal(
      guardianPublicTypeId({
        accountType: 'librarian',
        birthdate: '2014-06-15',
        storedPublicTypeId: '3',
        publicTypes: BANDS,
        today: TODAY,
      }),
      '3',
    );
  });

  it('requires a birth date on reader create and not on reader edit', () => {
    assert.equal(readerBirthDateRequired('create', 'reader'), true);
    assert.equal(readerBirthDateRequired('edit', 'reader'), false);
    assert.equal(readerBirthDateRequired('edit', 'READER'), false);
    assert.equal(readerBirthDateRequired('edit', 'librarian'), true);
    assert.equal(readerBirthDateRequired('create', 'librarian'), true);
  });

  it('omits publicType for a reader and keeps it for other account types', async () => {
    const { buildUserWriteBody } = await import('./userWriteBody.ts');
    const reader = buildUserWriteBody(formData());
    assert.equal('publicType' in reader, false);
    assert.equal(reader.accountType, 'reader');
    assert.equal(reader.birthdate, '2010-06-15');

    const librarian = buildUserWriteBody(formData({ accountType: 'librarian', publicType: '3' }));
    assert.equal(librarian.publicType, '3');
  });

  it('maps the two 422 codes onto French and English birth-date messages', async () => {
    const frI18n = await i18nFor('fr');
    const enI18n = await i18nFor('en');
    const required = httpError('birth_date_required', 'Birth date is required to determine the reader public type');
    const noBand = httpError(
      'no_public_type_for_age',
      'No public type matches age 13. An administrator must complete the age bands.',
    );
    assert.equal(readerBirthDateFieldError(required, frI18n.t.bind(frI18n)), FR_REQUIRED);
    assert.equal(readerBirthDateFieldError(required, enI18n.t.bind(enI18n)), EN_REQUIRED);
    assert.equal(readerBirthDateFieldError(noBand, frI18n.t.bind(frI18n)), FR_NO_BAND);
    assert.equal(readerBirthDateFieldError(noBand, enI18n.t.bind(enI18n)), EN_NO_BAND);
    assert.equal(getApiErrorMessage(noBand, frI18n.t.bind(frI18n)), FR_NO_BAND);
    assert.equal(readerBirthDateFieldError(httpError('validation_error', 'nope'), frI18n.t.bind(frI18n)), null);
    assert.equal(FR_REQUIRED.includes('lecteur'), true);
    assert.equal(FR_NO_BAND.includes('Paramètres > Types de public'), true);
  });

  it('hides the public type selector for a new reader and requires the birth date', async () => {
    const html = await renderEditor(await i18nFor('fr'), { mode: 'create' });
    assert.equal(html.includes('<select id="reader-form-public-type"'), false);
    assert.match(html, /<div id="reader-form-public-type">/);
    assert.match(html, new RegExp(FR_HINT.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
    assert.equal(html.includes(FR_MISSING), false);
    const birth = inputById(html, 'reader-form-birthdate');
    assert.match(birth, /\brequired(?:=""|="")?/);
    assert.match(birth, /type="date"/);
  });

  it('keeps an existing reader editable without a birth date and shows the stored type', async () => {
    const html = await renderEditor(await i18nFor('fr'), {
      mode: 'edit',
      user: {
        id: '9',
        username: 'ada',
        accountType: 'reader',
        publicType: '1',
        birthdate: '',
        birthDateMissing: true,
      } as User,
    });
    assert.equal(html.includes('<select id="reader-form-public-type"'), false);
    assert.match(html, /Enfant/);
    assert.match(html, new RegExp(FR_MISSING));
    const birth = inputById(html, 'reader-form-birthdate');
    assert.equal(/\brequired\b/.test(birth), false);
  });

  it('still lets staff choose a public type for a non-reader', async () => {
    const html = await renderEditor(await i18nFor('en'), {
      mode: 'edit',
      user: {
        id: '4',
        username: 'staff',
        accountType: 'librarian',
        publicType: '3',
        birthdate: '1980-01-01',
      } as User,
    });
    assert.match(html, /<select id="reader-form-public-type"/);
    assert.equal(html.includes(FR_MISSING), false);
    assert.match(inputById(html, 'reader-form-birthdate'), /\brequired\b/);
  });

  it('shows a read-only public type on the reader profile and none for staff', async () => {
    const readerHtml = await renderProfile(await i18nFor('fr'), {
      id: '9',
      username: 'ada',
      login: 'ada',
      accountType: 'reader',
      publicType: 'child',
      birthDateMissing: true,
      birthdate: '',
      firstname: 'Ada',
      lastname: 'Lovelace',
    } as User);
    assert.match(readerHtml, /id="profile-public-type"/);
    assert.equal(readerHtml.includes('<select id="profile-public-type"'), false);
    assert.match(readerHtml, /Enfant/);
    assert.match(readerHtml, new RegExp(FR_MISSING));
    const birth = inputById(readerHtml, 'profile-birthdate');
    assert.equal(/\brequired\b/.test(birth), false);

    const staffHtml = await renderProfile(await i18nFor('fr'), {
      id: '4',
      username: 'staff',
      login: 'staff',
      accountType: 'librarian',
      publicType: 'staff',
      firstname: 'Grace',
      lastname: 'Hopper',
    } as User);
    assert.equal(staffHtml.includes('id="profile-public-type"'), false);
    assert.equal(staffHtml.includes('<select id="profile-public-type"'), false);
    assert.equal(staffHtml.includes(FR_MISSING), false);
  });
});
