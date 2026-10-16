import { useEffect, useState, type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useLocation, useSearchParams } from 'react-router-dom';
import {
  User,
  Mail,
  Phone,
  KeyRound,
  Save,
  Eye,
  EyeOff,
  CheckCircle,
  AlertCircle,
  MapPin,
  Calendar,
  Briefcase,
  AtSign,
  Shield,
  ShieldOff,
  Smartphone,
  Copy,
  Check,
  Palette,
  ShoppingBag,
} from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { Card, CardHeader, Button, Input, Modal } from '@/components/common';
import ThemeSwitcher from '@/components/common/ThemeSwitcher';
import PatronSuggestionsSection from '@/components/suggestions/PatronSuggestionsSection';
import EventsConsentFields from '@/components/users/EventsConsentFields';
import ReaderPublicTypeReadout from '@/components/users/ReaderPublicTypeReadout';
import { useAuth } from '@/contexts/AuthContext';
import { useLanguage } from '@/contexts/LanguageContext';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import api from '@/services/api';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import type { SupportedLanguage } from '@/locales';
import { canReadUsers, ownContactOnly, resolveUsersRights, type TwoFactorMethod, type User as UserProfile } from '@/types';
import { ownContactPayload } from '@/utils/ownContactForm';
import { profileChildAccountPending, profileIsChildAccount } from '@/utils/legalGuardian';
import { selfProfilePayload } from '@/utils/profileUpdate';
import { isReaderAccount, readerBirthDateFieldError } from '@/utils/readerPublicType';
import { OCCUPATION_OPTIONS } from '@/utils/codeLabels';
import { APP_PAGE_CLASS } from '@/utils/appPageLayout';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import {
  FORM_FIELD_PAIR_ROW,
  FORM_FIELD_STACK,
  FORM_FIELD_WIDTH,
} from '@/utils/formLayout';

const PROFILE_TAB_IDS = ['personal', 'preferences', 'security', 'suggestions'] as const;
type ProfileTab = (typeof PROFILE_TAB_IDS)[number];

function isProfileTab(value: string | null): value is ProfileTab {
  return value != null && PROFILE_TAB_IDS.includes(value as ProfileTab);
}

function profileFormFromUser(user: UserProfile | null) {
  return {
    firstname: user?.firstname || '',
    lastname: user?.lastname || '',
    email: user?.email || '',
    phone: user?.phone || '',
    login: user?.login || user?.username || '',
    addrStreet: user?.addrStreet || '',
    addrZipCode: user?.addrZipCode?.toString() || '',
    addrCity: user?.addrCity || '',
    birthdate: user?.birthdate || '',
    eventsConsent: user?.eventsConsent === true,
    receiveReminders: false,
  };
}

export default function ProfilePage() {
  const { t } = useTranslation();
  const [searchParams, setSearchParams] = useSearchParams();
  const location = useLocation();
  const { user, refreshProfile } = useAuth();
  const { language, setLanguage, availableLanguages, languageNames, languageFlags } = useLanguage();
  const { data: publicTypes = [], isLoading: publicTypesLoading } = usePublicTypesQuery();
  const isChildAccount = profileIsChildAccount(user?.publicType, publicTypes);
  const childAccountPending = profileChildAccountPending(user?.publicType, publicTypesLoading);
  const token = api.getToken();
  const contactOnly = ownContactOnly(user, token);
  const usersLevel = resolveUsersRights(user, token);
  const canLoadOwnRecord = usersLevel === 'o' || usersLevel === 'r' || usersLevel === 'w';
  const ownRecordQuery = useQuery({
    queryKey: ['own-user-record', user?.id],
    queryFn: () => api.getUser(user!.id),
    enabled: Boolean(user?.id) && canLoadOwnRecord,
  });
  const cardNumber = ownRecordQuery.data?.barcode?.trim() || '';

  // Profile form state
  const [profileData, setProfileData] = useState(() => profileFormFromUser(user));
  const [isUpdatingProfile, setIsUpdatingProfile] = useState(false);
  const [profileSuccess, setProfileSuccess] = useState(false);
  const [profileError, setProfileError] = useState('');
  const [birthDateError, setBirthDateError] = useState('');

  // Password form state
  const [passwordData, setPasswordData] = useState({
    currentPassword: '',
    newPassword: '',
    confirmPassword: '',
  });
  const [showCurrentPassword, setShowCurrentPassword] = useState(false);
  const [showNewPassword, setShowNewPassword] = useState(false);
  const [isUpdatingPassword, setIsUpdatingPassword] = useState(false);
  const [passwordSuccess, setPasswordSuccess] = useState(false);
  const [passwordError, setPasswordError] = useState('');

  // 2FA state
  const [show2FAModal, setShow2FAModal] = useState(false);
  const [twoFAMethod, setTwoFAMethod] = useState<TwoFactorMethod>('totp');
  const [twoFASetupData, setTwoFASetupData] = useState<{
    provisioningUri?: string;
    recoveryCodes: string[];
  } | null>(null);
  const [isSettingUp2FA, setIsSettingUp2FA] = useState(false);
  const [isDisabling2FA, setIsDisabling2FA] = useState(false);
  const [twoFAError, setTwoFAError] = useState('');
  const [copiedRecoveryCodes, setCopiedRecoveryCodes] = useState(false);

  const tabParam = searchParams.get('tab');
  const activeTab: ProfileTab = isProfileTab(tabParam) ? tabParam : 'personal';

  const selectTab = (id: ProfileTab) => {
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('tab', id);
        return next;
      },
      { replace: true },
    );
  };

  useEffect(() => {
    if (location.hash !== '#suggestions') return;
    setSearchParams(
      (prev) => {
        const next = new URLSearchParams(prev);
        next.set('tab', 'suggestions');
        return next;
      },
      { replace: true },
    );
    window.history.replaceState(null, '', `${location.pathname}?tab=suggestions`);
  }, [location.hash, location.pathname, setSearchParams]);

  const profileTabs: { id: ProfileTab; label: string; icon: ReactNode }[] = [
    { id: 'personal', label: t('profile.tabs.personal'), icon: <User className="h-5 w-5" aria-hidden /> },
    {
      id: 'preferences',
      label: t('profile.tabs.preferences'),
      icon: <Palette className="h-5 w-5" aria-hidden />,
    },
    { id: 'security', label: t('profile.tabs.security'), icon: <Shield className="h-5 w-5" aria-hidden /> },
    {
      id: 'suggestions',
      label: t('profile.tabs.suggestions'),
      icon: <ShoppingBag className="h-5 w-5" aria-hidden />,
    },
  ];

  const [prevProfileUser, setPrevProfileUser] = useState(user);
  if (user && user !== prevProfileUser) {
    setPrevProfileUser(user);
    setProfileData(profileFormFromUser(user));
  }
  const ownRecord = ownRecordQuery.data;
  const [prevOwnRecord, setPrevOwnRecord] = useState(ownRecord);
  if (ownRecord && ownRecord !== prevOwnRecord) {
    setPrevOwnRecord(ownRecord);
    setProfileData((current) => ({
      ...current,
      receiveReminders: ownRecord.receiveReminders === true,
    }));
  }

  const handleProfileSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user) return;

    setIsUpdatingProfile(true);
    setProfileError('');
    setBirthDateError('');
    setProfileSuccess(false);

    try {
      if (contactOnly) {
        await api.updateUser(
          user.id,
          ownContactPayload(profileData, {
            includeEventsConsent: !isChildAccount && !childAccountPending,
            includeReminders: ownRecord != null,
          }) as Partial<UserProfile>,
        );
      } else {
        await api.updateProfile(
          selfProfilePayload(profileData, !isChildAccount && !childAccountPending),
        );
      }
      await refreshProfile();
      setProfileSuccess(true);
      setTimeout(() => setProfileSuccess(false), 3000);
    } catch (error) {
      console.error('Error updating profile:', error);
      const birthField = readerBirthDateFieldError(error, t);
      if (birthField) {
        setBirthDateError(birthField);
      } else {
        setProfileError(t('profile.updateError'));
      }
    } finally {
      setIsUpdatingProfile(false);
    }
  };

  const handlePasswordSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!user) return;

    // Validation
    if (passwordData.newPassword !== passwordData.confirmPassword) {
      setPasswordError(t('profile.passwordMismatch'));
      return;
    }

    if (passwordData.newPassword.length < 8) {
      setPasswordError(t('profile.passwordTooShort'));
      return;
    }

    setIsUpdatingPassword(true);
    setPasswordError('');
    setPasswordSuccess(false);

    try {
      await api.updateProfile({
        currentPassword: passwordData.currentPassword,
        newPassword: passwordData.newPassword,
      });
      setPasswordSuccess(true);
      setPasswordData({
        currentPassword: '',
        newPassword: '',
        confirmPassword: '',
      });
      setTimeout(() => setPasswordSuccess(false), 3000);
    } catch (error) {
      console.error('Error updating password:', error);
      setPasswordError(t('profile.updateError'));
    } finally {
      setIsUpdatingPassword(false);
    }
  };

  const handleLanguageChange = async (newLang: SupportedLanguage) => {
    await setLanguage(newLang);
  };

  const handleSetup2FA = async () => {
    setIsSettingUp2FA(true);
    setTwoFAError('');
    
    try {
      const response = await api.setup2FA({ method: twoFAMethod });
      setTwoFASetupData(response);
    } catch (error) {
      console.error('Error setting up 2FA:', error);
      setTwoFAError(t('profile.2fa.setupError'));
    } finally {
      setIsSettingUp2FA(false);
    }
  };

  const handleDisable2FA = async () => {
    setIsDisabling2FA(true);
    setTwoFAError('');
    
    try {
      await api.disable2FA();
      await refreshProfile();
      setShow2FAModal(false);
    } catch (error) {
      console.error('Error disabling 2FA:', error);
      setTwoFAError(t('profile.2fa.disableError'));
    } finally {
      setIsDisabling2FA(false);
    }
  };

  const handleCopyRecoveryCodes = () => {
    if (twoFASetupData?.recoveryCodes) {
      navigator.clipboard.writeText(twoFASetupData.recoveryCodes.join('\n'));
      setCopiedRecoveryCodes(true);
      setTimeout(() => setCopiedRecoveryCodes(false), 2000);
    }
  };

  const handleClose2FAModal = async () => {
    if (twoFASetupData) {
      // 2FA was set up successfully, refresh profile
      await refreshProfile();
    }
    setShow2FAModal(false);
    setTwoFASetupData(null);
    setTwoFAError('');
    setCopiedRecoveryCodes(false);
  };

  return (
    <div className={APP_PAGE_CLASS}>
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('profile.title')}</h1>
        <p className="text-gray-500 dark:text-gray-400 mt-1">{t('profile.subtitle')}</p>
      </div>

      <div className="flex items-center gap-4 rounded-2xl border border-gray-200/80 dark:border-gray-800/80 bg-white dark:bg-gray-900 px-4 py-4 sm:px-6">
        <div className="flex-shrink-0 h-14 w-14 rounded-full bg-indigo-100 dark:bg-indigo-900 flex items-center justify-center">
          <span className="text-xl font-bold text-indigo-600 dark:text-indigo-400">
            {user?.firstname?.[0] || user?.username?.[0] || '?'}
            {user?.lastname?.[0] || ''}
          </span>
        </div>
        <div className="min-w-0">
          <p className="text-lg font-semibold text-gray-900 dark:text-white truncate">
            {user?.firstname} {user?.lastname}
          </p>
          <p className="text-sm text-gray-500 dark:text-gray-400 truncate">
            {localizedAccountTypeLabel(t, user?.accountType)} • @{user?.username}
          </p>
        </div>
      </div>

      <div className="rounded-2xl border border-gray-200/80 dark:border-gray-800/80 bg-gray-50/50 dark:bg-gray-900/30 p-1.5 sm:p-2">
        <nav className="flex flex-wrap gap-1" role="tablist" aria-label={t('profile.tabs.ariaLabel')}>
          {profileTabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              id={`profile-tab-${tab.id}`}
              aria-selected={activeTab === tab.id}
              aria-controls={`profile-panel-${tab.id}`}
              onClick={() => selectTab(tab.id)}
              className={`flex items-center gap-2 rounded-xl px-3 py-2.5 text-sm font-medium transition-colors ${
                activeTab === tab.id
                  ? 'bg-white dark:bg-gray-800 text-amber-700 dark:text-amber-400 shadow-sm border border-gray-200/70 dark:border-gray-700/70'
                  : 'text-gray-600 dark:text-gray-400 hover:bg-white/60 dark:hover:bg-gray-800/40 hover:text-gray-900 dark:hover:text-gray-200 border border-transparent'
              }`}
            >
              {tab.icon}
              {tab.label}
            </button>
          ))}
        </nav>
      </div>

      {activeTab === 'personal' && (
      <div role="tabpanel" id="profile-panel-personal" aria-labelledby="profile-tab-personal">
      <Card>
        <form onSubmit={handleProfileSubmit} className="space-y-6">
          {/* Personal Information Section */}
          <div>
            <CardHeader title={t('profile.personalInfo')} headingId="profile-personal-heading" />
            <fieldset
              aria-labelledby="profile-personal-heading"
              className={`${FORM_FIELD_STACK} mt-4 min-w-0 border-0 p-0`}
            >
              <div className={FORM_FIELD_PAIR_ROW}>
                <Input
                  label={t('profile.firstName')}
                  value={profileData.firstname}
                  onChange={(e) => setProfileData({ ...profileData, firstname: e.target.value })}
                  leftIcon={<User className="h-4 w-4" />}
                  readOnly={contactOnly}
                  autoComplete="given-name"
                />
                <Input
                  label={t('profile.lastName')}
                  value={profileData.lastname}
                  onChange={(e) => setProfileData({ ...profileData, lastname: e.target.value })}
                  leftIcon={<User className="h-4 w-4" />}
                  readOnly={contactOnly}
                  autoComplete="family-name"
                />
              </div>
              <Input
                label={t('profile.loginUsername')}
                value={profileData.login}
                onChange={(e) => setProfileData({ ...profileData, login: e.target.value })}
                leftIcon={<AtSign className="h-4 w-4" />}
                hint={contactOnly ? undefined : t('profile.loginHint')}
                readOnly={contactOnly}
                autoComplete="username"
                wrapperClassName={FORM_FIELD_WIDTH.long}
              />
              <Input
                label={t('profile.email')}
                type="email"
                value={profileData.email}
                onChange={(e) => setProfileData({ ...profileData, email: e.target.value })}
                leftIcon={<Mail className="h-4 w-4" />}
                autoComplete="email"
                wrapperClassName={FORM_FIELD_WIDTH.email}
              />
              <Input
                label={t('profile.phone')}
                type="tel"
                value={profileData.phone}
                onChange={(e) => setProfileData({ ...profileData, phone: e.target.value })}
                leftIcon={<Phone className="h-4 w-4" />}
                autoComplete="tel"
                wrapperClassName={FORM_FIELD_WIDTH.long}
              />
              <Input
                label={t('profile.birthdate')}
                type="date"
                value={profileData.birthdate}
                onChange={(e) => setProfileData({ ...profileData, birthdate: e.target.value })}
                leftIcon={<Calendar className="h-4 w-4" />}
                readOnly={contactOnly}
                autoComplete="bday"
                wrapperClassName={FORM_FIELD_WIDTH.short}
              />
              {cardNumber ? (
                <Input
                  label={t('profile.barcode')}
                  value={cardNumber}
                  readOnly
                  leftIcon={<User className="h-4 w-4" />}
                  wrapperClassName={FORM_FIELD_WIDTH.medium}
                />
              ) : null}
              <div className={FORM_FIELD_WIDTH.long}>
                <label className={formLabelClass()} htmlFor="profile-occupation">
                  {t('profile.occupation')}
                </label>
                <div className="relative">
                  <div className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400">
                    <Briefcase className="h-4 w-4" />
                  </div>
                  <select
                    id="profile-occupation"
                    disabled
                    className={formControlClass({ className: 'w-full pl-10' })}
                  >
                    <option value="">{t('common.select')}</option>
                    {OCCUPATION_OPTIONS.map((opt) => (
                      <option key={opt.value} value={opt.value}>
                        {t(opt.labelKey)}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {childAccountPending ? null : (
                <div className="max-w-xl pt-1">
                  <EventsConsentFields
                    name="profile-events-consent"
                    value={profileData.eventsConsent}
                    onChange={(next) => setProfileData({ ...profileData, eventsConsent: next })}
                    isChild={isChildAccount}
                    guardianId={user?.guardianId}
                    showGuardianLink={canReadUsers(user, token)}
                    recorded={{
                      consent: user?.eventsConsent,
                      at: user?.eventsConsentAt,
                      source: user?.eventsConsentSource,
                    }}
                  />
                </div>
              )}

              {contactOnly && ownRecord ? (
                <label className="flex max-w-xl items-start gap-3 text-sm text-gray-700 dark:text-gray-300">
                  <input
                    type="checkbox"
                    className="mt-0.5"
                    checked={profileData.receiveReminders}
                    onChange={(e) =>
                      setProfileData({ ...profileData, receiveReminders: e.target.checked })
                    }
                  />
                  <span>
                    <span className="font-medium">{t('profile.receiveReminders')}</span>
                    <span className="mt-0.5 block text-gray-500 dark:text-gray-400">
                      {t('profile.receiveRemindersHint')}
                    </span>
                  </span>
                </label>
              ) : null}
            </fieldset>
          </div>

          {/* Address Section */}
          <div>
            <CardHeader title={t('profile.address')} headingId="profile-address-heading" />
            <fieldset
              aria-labelledby="profile-address-heading"
              className={`${FORM_FIELD_STACK} mt-4 min-w-0 border-0 p-0`}
            >
              <Input
                label={t('profile.street')}
                value={profileData.addrStreet}
                onChange={(e) => setProfileData({ ...profileData, addrStreet: e.target.value })}
                leftIcon={<MapPin className="h-4 w-4" />}
                autoComplete="street-address"
                wrapperClassName={FORM_FIELD_WIDTH.street}
              />
              <div className={FORM_FIELD_PAIR_ROW}>
                <Input
                  label={t('profile.zipCode')}
                  type="text"
                  inputMode="numeric"
                  value={profileData.addrZipCode}
                  onChange={(e) => setProfileData({ ...profileData, addrZipCode: e.target.value })}
                  autoComplete="postal-code"
                  wrapperClassName={FORM_FIELD_WIDTH.zip}
                />
                <Input
                  label={t('profile.city')}
                  value={profileData.addrCity}
                  onChange={(e) => setProfileData({ ...profileData, addrCity: e.target.value })}
                  autoComplete="address-level2"
                  wrapperClassName={FORM_FIELD_WIDTH.long}
                />
              </div>
            </fieldset>
          </div>

          {profileError && (
            <div className="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
              <AlertCircle className="h-4 w-4" />
              {profileError}
            </div>
          )}

          {profileSuccess && (
            <div className="flex items-center gap-2 text-green-600 dark:text-green-400 text-sm">
              <CheckCircle className="h-4 w-4" />
              {t('profile.profileUpdated')}
            </div>
          )}

          <div className="flex justify-end pt-2">
            <Button
              type="submit"
              isLoading={isUpdatingProfile}
              leftIcon={<Save className="h-4 w-4" />}
            >
              {t('common.save')}
            </Button>
          </div>
        </form>
      </Card>
      </div>
      )}

      {activeTab === 'preferences' && (
      <div
        role="tabpanel"
        id="profile-panel-preferences"
        aria-labelledby="profile-tab-preferences"
        className="space-y-6"
      >
        <Card>
          <CardHeader title={t('profile.language')} subtitle={t('profile.languageHint')} />
          <div className="flex flex-wrap gap-3">
            {availableLanguages.map((lang) => (
              <button
                key={lang}
                type="button"
                onClick={() => handleLanguageChange(lang)}
                aria-pressed={language === lang}
                className={`
                  flex min-w-[8.5rem] items-center gap-2 rounded-lg border-2 p-3 transition-all
                  ${language === lang
                    ? 'border-indigo-500 bg-indigo-50 dark:bg-indigo-950'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                  }
                `}
              >
                <span className="text-xl">{languageFlags[lang]}</span>
                <span className={`text-sm font-medium ${
                  language === lang
                    ? 'text-indigo-700 dark:text-indigo-300'
                    : 'text-gray-700 dark:text-gray-300'
                }`}>
                  {languageNames[lang]}
                </span>
              </button>
            ))}
          </div>
        </Card>
        <Card>
          <CardHeader title={t('profile.theme')} subtitle={t('profile.themeHint')} />
          <ThemeSwitcher />
        </Card>
      </div>
      )}

      {activeTab === 'security' && (
      <div
        role="tabpanel"
        id="profile-panel-security"
        aria-labelledby="profile-tab-security"
        className="space-y-6"
      >
      <Card>
        <CardHeader
          title={t('profile.changePassword')}
          subtitle={t('profile.passwordHint')}
        />

        <form onSubmit={handlePasswordSubmit} className="space-y-4">
          <div className={FORM_FIELD_STACK}>
            <div className={`relative ${FORM_FIELD_WIDTH.password}`}>
              <Input
                label={t('auth.currentPassword')}
                type={showCurrentPassword ? 'text' : 'password'}
                value={passwordData.currentPassword}
                onChange={(e) => setPasswordData({ ...passwordData, currentPassword: e.target.value })}
                leftIcon={<KeyRound className="h-4 w-4" />}
                autoComplete="current-password"
              />
              <button
                type="button"
                onClick={() => setShowCurrentPassword(!showCurrentPassword)}
                aria-label={
                  showCurrentPassword ? t('auth.hidePassword') : t('auth.showPassword')
                }
                className="absolute right-3 top-8 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                {showCurrentPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            <div className={`relative ${FORM_FIELD_WIDTH.password}`}>
              <Input
                label={t('auth.newPassword')}
                type={showNewPassword ? 'text' : 'password'}
                value={passwordData.newPassword}
                onChange={(e) => setPasswordData({ ...passwordData, newPassword: e.target.value })}
                leftIcon={<KeyRound className="h-4 w-4" />}
                autoComplete="new-password"
              />
              <button
                type="button"
                onClick={() => setShowNewPassword(!showNewPassword)}
                aria-label={showNewPassword ? t('auth.hidePassword') : t('auth.showPassword')}
                className="absolute right-3 top-8 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
              >
                {showNewPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            <Input
              label={t('auth.confirmPassword')}
              type={showNewPassword ? 'text' : 'password'}
              value={passwordData.confirmPassword}
              onChange={(e) => setPasswordData({ ...passwordData, confirmPassword: e.target.value })}
              leftIcon={<KeyRound className="h-4 w-4" />}
              autoComplete="new-password"
              wrapperClassName={FORM_FIELD_WIDTH.password}
            />
          </div>

          {passwordError && (
            <div className="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
              <AlertCircle className="h-4 w-4" />
              {passwordError}
            </div>
          )}

          {passwordSuccess && (
            <div className="flex items-center gap-2 text-green-600 dark:text-green-400 text-sm">
              <CheckCircle className="h-4 w-4" />
              {t('profile.passwordChanged')}
            </div>
          )}

          <div className="flex justify-end pt-2">
            <Button
              type="submit"
              isLoading={isUpdatingPassword}
              disabled={!passwordData.newPassword || !passwordData.confirmPassword}
              leftIcon={<KeyRound className="h-4 w-4" />}
            >
              {t('profile.changePassword')}
            </Button>
          </div>
        </form>
      </Card>

      <Card>
        <CardHeader
          title={t('profile.2fa.title')}
          subtitle={t('profile.2fa.subtitle')}
        />

        <div className="space-y-4">
          {/* Current 2FA status */}
          <div className="flex max-w-2xl flex-col gap-4 rounded-lg bg-gray-50 p-4 dark:bg-gray-800/50 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-3">
              {user?.twoFactorEnabled ? (
                <>
                  <div className="p-2 rounded-full bg-green-100 dark:bg-green-900/30">
                    <Shield className="h-5 w-5 text-green-600 dark:text-green-400" />
                  </div>
                  <div>
                    <p className="font-medium text-gray-900 dark:text-white">
                      {t('profile.2fa.enabled')}
                    </p>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                      {user.twoFactorMethod === 'email' 
                        ? t('profile.2fa.methodEmail')
                        : t('profile.2fa.methodTotp')}
                    </p>
                  </div>
                </>
              ) : (
                <>
                  <div className="p-2 rounded-full bg-gray-100 dark:bg-gray-700">
                    <ShieldOff className="h-5 w-5 text-gray-400" />
                  </div>
                  <div>
                    <p className="font-medium text-gray-900 dark:text-white">
                      {t('profile.2fa.disabled')}
                    </p>
                    <p className="text-sm text-gray-500 dark:text-gray-400">
                      {t('profile.2fa.disabledHint')}
                    </p>
                  </div>
                </>
              )}
            </div>

            <Button
              variant={user?.twoFactorEnabled ? 'secondary' : 'primary'}
              onClick={() => setShow2FAModal(true)}
              leftIcon={user?.twoFactorEnabled 
                ? <ShieldOff className="h-4 w-4" /> 
                : <Shield className="h-4 w-4" />}
            >
              {user?.twoFactorEnabled 
                ? t('profile.2fa.disable')
                : t('profile.2fa.enable')}
            </Button>
          </div>
        </div>
      </Card>
      </div>
      )}

      {activeTab === 'suggestions' && (
        <div
          role="tabpanel"
          id="profile-panel-suggestions"
          aria-labelledby="profile-tab-suggestions"
        >
          <PatronSuggestionsSection />
        </div>
      )}

      <Modal
        isOpen={show2FAModal}
        onClose={handleClose2FAModal}
        title={user?.twoFactorEnabled ? t('profile.2fa.disableTitle') : t('profile.2fa.setupTitle')}
        footer={
          user?.twoFactorEnabled ? (
            <div className="flex justify-end gap-3">
              <Button variant="secondary" onClick={handleClose2FAModal}>
                {t('common.cancel')}
              </Button>
              <Button
                variant="danger"
                onClick={handleDisable2FA}
                isLoading={isDisabling2FA}
                leftIcon={<ShieldOff className="h-4 w-4" />}
              >
                {t('profile.2fa.confirmDisable')}
              </Button>
            </div>
          ) : twoFASetupData ? (
            <div className="flex justify-end">
              <Button onClick={handleClose2FAModal}>
                {t('common.close')}
              </Button>
            </div>
          ) : (
            <div className="flex justify-end gap-3">
              <Button variant="secondary" onClick={handleClose2FAModal}>
                {t('common.cancel')}
              </Button>
              <Button
                onClick={handleSetup2FA}
                isLoading={isSettingUp2FA}
                leftIcon={<Shield className="h-4 w-4" />}
              >
                {t('profile.2fa.setup')}
              </Button>
            </div>
          )
        }
      >
        {user?.twoFactorEnabled ? (
          <div className="space-y-4">
            <p className="text-gray-600 dark:text-gray-400">
              {t('profile.2fa.disableWarning')}
            </p>
            {twoFAError && (
              <div className="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                <AlertCircle className="h-4 w-4" />
                {twoFAError}
              </div>
            )}
          </div>
        ) : twoFASetupData ? (
          <div className="space-y-6">
            {twoFASetupData.provisioningUri && (
              <div className="text-center">
                <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
                  {t('profile.2fa.scanQR')}
                </p>
                <div className="inline-block p-4 bg-white rounded-lg">
                  <QRCodeSVG 
                    value={twoFASetupData.provisioningUri}
                    size={192}
                    level="M"
                    includeMargin={false}
                  />
                </div>
              </div>
            )}

            <div>
              <div className="flex items-center justify-between mb-2">
                <p className="text-sm font-medium text-gray-700 dark:text-gray-300">
                  {t('profile.2fa.recoveryCodes')}
                </p>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={handleCopyRecoveryCodes}
                  leftIcon={copiedRecoveryCodes ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
                >
                  {copiedRecoveryCodes ? t('common.copied') : t('common.copy')}
                </Button>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
                {t('profile.2fa.recoveryCodesHint')}
              </p>
              <div className="grid grid-cols-2 gap-2 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg font-mono text-sm">
                {twoFASetupData.recoveryCodes.map((code, index) => (
                  <div key={index} className="text-gray-700 dark:text-gray-300">
                    {code}
                  </div>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="space-y-6">
            <p className="text-gray-600 dark:text-gray-400">
              {t('profile.2fa.chooseMethod')}
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <button
                onClick={() => setTwoFAMethod('totp')}
                className={`p-4 rounded-lg border-2 text-left transition-all ${
                  twoFAMethod === 'totp'
                    ? 'border-indigo-500 bg-indigo-50 dark:bg-indigo-950'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300'
                }`}
              >
                <Smartphone className={`h-6 w-6 mb-2 ${
                  twoFAMethod === 'totp' ? 'text-indigo-600 dark:text-indigo-400' : 'text-gray-400'
                }`} />
                <p className={`font-medium ${
                  twoFAMethod === 'totp' ? 'text-indigo-700 dark:text-indigo-300' : 'text-gray-700 dark:text-gray-300'
                }`}>
                  {t('profile.2fa.totpOption')}
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                  {t('profile.2fa.totpDescription')}
                </p>
              </button>

              <button
                onClick={() => setTwoFAMethod('email')}
                className={`p-4 rounded-lg border-2 text-left transition-all ${
                  twoFAMethod === 'email'
                    ? 'border-indigo-500 bg-indigo-50 dark:bg-indigo-950'
                    : 'border-gray-200 dark:border-gray-700 hover:border-gray-300'
                }`}
              >
                <Mail className={`h-6 w-6 mb-2 ${
                  twoFAMethod === 'email' ? 'text-indigo-600 dark:text-indigo-400' : 'text-gray-400'
                }`} />
                <p className={`font-medium ${
                  twoFAMethod === 'email' ? 'text-indigo-700 dark:text-indigo-300' : 'text-gray-700 dark:text-gray-300'
                }`}>
                  {t('profile.2fa.emailOption')}
                </p>
                <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                  {t('profile.2fa.emailDescription')}
                </p>
              </button>
            </div>

            {twoFAError && (
              <div className="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                <AlertCircle className="h-4 w-4" />
                {twoFAError}
              </div>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
}
