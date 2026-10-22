import { useState, useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { invalidateScheduleQueries, useScheduleAdminQuery } from '@/hooks/common/useScheduleQueries';
import { useTranslation } from 'react-i18next';
import {
  Building2,
  Clock,
  Plus,
  Trash2,
  ChevronDown,
  ChevronRight,
  Pencil,
  CalendarOff,
  X,
  Check,
  Phone,
} from 'lucide-react';
import { Card, Button, Input, Modal, CountrySelect, LibraryTimezoneControl, SectionTabBar } from '@/components/common';
import { useLibraryInfoQuery, useUpdateLibraryInfoMutation } from '@/hooks/common/useLibraryInfoQuery';
import { LANGUAGE_NAMES, PUBLIC_LANGUAGE_OPTIONS } from '@/locales';
import api from '@/services/api';
import { getApiErrorMessage, libraryInfoLanguageFieldError, libraryInfoTimezoneFieldError } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { libraryLanguageChoice, libraryLanguageForUpdate } from '@/utils/uiLanguage';
import {
  applyCountryChange,
  countryCodeFromStored,
  timezoneChoiceRequired,
} from '@/utils/countryTimezone';
import type {
  LibraryInfo,
  SchedulePeriod,
  CreateSchedulePeriod,
  UpdateSchedulePeriod,
  CreateScheduleSlot,
  CreateScheduleClosure,
} from '@/types';

const DAYS_OF_WEEK = [0, 1, 2, 3, 4, 5, 6] as const;

interface LibraryGeneralForm {
  name: string;
  addrLine1: string;
  addrLine2: string;
  addrPostcode: string;
  addrCity: string;
  addrCountry: string;
  email: string;
  phones: string[];
  timezone: string;
  /** `''` is “not set” and is saved as JSON `null`. */
  language: string;
}

function libraryInfoToForm(info: LibraryInfo | undefined): LibraryGeneralForm {
  return {
    name: info?.name ?? '',
    addrLine1: info?.addrLine1 ?? '',
    addrLine2: info?.addrLine2 ?? '',
    addrPostcode: info?.addrPostcode ?? '',
    addrCity: info?.addrCity ?? '',
    addrCountry: countryCodeFromStored(info?.addrCountry),
    email: info?.email ?? '',
    phones: info?.phones ?? [],
    timezone: info?.timezone?.trim() ?? '',
    language: libraryLanguageChoice(info?.language),
  };
}

// ─── General Tab ─────────────────────────────────────────────────────────────

function GeneralTab() {
  const { t } = useTranslation();
  const { data, isLoading, isError, refetch } = useLibraryInfoQuery();
  const updateLibraryInfo = useUpdateLibraryInfoMutation();

  const [saveSuccess, setSaveSuccess] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [timezoneError, setTimezoneError] = useState<string | null>(null);
  const [countryError, setCountryError] = useState<string | null>(null);
  const [languageError, setLanguageError] = useState<string | null>(null);

  // Seed from a cache hit on the first render. Synced starts unset so a library
  // info payload already in the query cache is copied into the draft (same class
  // of bug as the settings forms: initialising the synced ref to `data` skips it).
  const [form, setForm] = useState<LibraryGeneralForm>(() => libraryInfoToForm(data));
  const [syncedInfo, setSyncedInfo] = useState<LibraryInfo | undefined>(undefined);
  if (data !== syncedInfo) {
    setSyncedInfo(data);
    if (data) {
      setForm(libraryInfoToForm(data));
      setTimezoneError(null);
      setCountryError(null);
      setLanguageError(null);
    }
  }

  const handleSave = async () => {
    setSaveError(null);
    setTimezoneError(null);
    setCountryError(null);
    setLanguageError(null);
    setSaveSuccess(false);
    if (!/^[A-Z]{2}$/.test(form.addrCountry)) {
      setCountryError(t('library.general.countryRequired'));
      return;
    }
    if (timezoneChoiceRequired(form.addrCountry, form.timezone)) {
      setTimezoneError(t('library.general.timezoneRequired'));
      return;
    }
    try {
      await updateLibraryInfo.mutateAsync({
        name: form.name || null,
        addrLine1: form.addrLine1 || null,
        addrLine2: form.addrLine2 || null,
        addrPostcode: form.addrPostcode || null,
        addrCity: form.addrCity || null,
        addrCountry: form.addrCountry,
        email: form.email || null,
        phones: form.phones.filter(Boolean),
        timezone: form.timezone.trim() || null,
        language: libraryLanguageForUpdate(form.language),
      });
      setSaveSuccess(true);
      setTimeout(() => setSaveSuccess(false), 3000);
    } catch (err: unknown) {
      const languageMessage = libraryInfoLanguageFieldError(err, t);
      if (languageMessage) {
        setLanguageError(languageMessage);
      } else {
        const timezoneMessage = libraryInfoTimezoneFieldError(err, t);
        if (timezoneMessage) setTimezoneError(timezoneMessage);
        else setSaveError(getApiErrorMessage(err, t));
      }
    }
  };

  const saving = updateLibraryInfo.isPending;

  const setPhone = (index: number, value: string) => {
    setForm((f) => {
      const phones = [...f.phones];
      phones[index] = value;
      return { ...f, phones };
    });
  };

  const addPhone = () => setForm((f) => ({ ...f, phones: [...f.phones, ''] }));

  const removePhone = (index: number) =>
    setForm((f) => ({ ...f, phones: f.phones.filter((_, i) => i !== index) }));

  const field = (
    labelKey: string,
    fieldName: keyof typeof form,
    opts?: { type?: string; placeholder?: string; half?: boolean }
  ) => (
    <div>
      <label className={formLabelClass()}>
        {t(labelKey)}
      </label>
      <Input
        type={opts?.type ?? 'text'}
        value={form[fieldName] as string}
        onChange={(e) => setForm((f) => ({ ...f, [fieldName]: e.target.value }))}
        placeholder={opts?.placeholder}
      />
    </div>
  );

  if (isLoading && !data) {
    return (
      <Card padding="none">
        <div className="flex justify-center py-12">
          <div className="h-8 w-8 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
        </div>
      </Card>
    );
  }

  if (isError && !data) {
    return (
      <Card padding="none">
        <div className="p-6 text-center space-y-3">
          <p className="text-red-600 dark:text-red-400 text-sm">{t('errors.generic')}</p>
          <Button size="sm" variant="ghost" onClick={() => void refetch()}>
            {t('common.retry')}
          </Button>
        </div>
      </Card>
    );
  }

  return (
    <Card padding="none">
      <div className="p-6 space-y-5 max-w-lg">
        {/* Feedback */}
        {saveSuccess && (
          <div
            role="status"
            className="flex items-center gap-2 p-3 rounded-lg bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400 text-sm"
          >
            <Check className="h-4 w-4 flex-shrink-0" aria-hidden="true" />
            {t('library.general.saveSuccess')}
          </div>
        )}
        {saveError && (
          <div className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
            {saveError}
          </div>
        )}

        {/* Name */}
        {field('library.general.name', 'name', { placeholder: t('library.general.namePlaceholder') })}

        <div>
          <label htmlFor="library-default-language" className={formLabelClass()}>
            {t('library.general.defaultLanguage')}
          </label>
          <select
            id="library-default-language"
            value={form.language}
            onChange={(event) => {
              setForm((current) => ({ ...current, language: event.target.value }));
              if (languageError) setLanguageError(null);
            }}
            className={formControlClass({ error: !!languageError, className: 'w-full' })}
            aria-invalid={languageError ? true : undefined}
            aria-describedby={
              languageError
                ? 'library-default-language-help library-default-language-error'
                : 'library-default-language-help'
            }
          >
            <option value="">{t('library.general.defaultLanguageUnset')}</option>
            {PUBLIC_LANGUAGE_OPTIONS.map((code) => (
              <option key={code} value={code}>
                {LANGUAGE_NAMES[code]}
              </option>
            ))}
          </select>
          <p id="library-default-language-help" className="mt-1 text-xs text-gray-500 dark:text-gray-400">
            {t('library.general.defaultLanguageHelp')}
          </p>
          {languageError && (
            <p
              id="library-default-language-error"
              className="mt-1 text-sm text-red-600 dark:text-red-400"
              role="alert"
            >
              {languageError}
            </p>
          )}
        </div>

        {/* Address */}
        <div className="space-y-3">
          <p className="text-sm font-semibold text-gray-700 dark:text-gray-300">
            {t('library.general.address')}
          </p>
          {field('library.general.addrLine1', 'addrLine1', { placeholder: t('library.general.addrLine1Placeholder') })}
          {field('library.general.addrLine2', 'addrLine2', { placeholder: t('library.general.addrLine2Placeholder') })}
          <div className="grid grid-cols-2 gap-3">
            {field('library.general.zipCode', 'addrPostcode', { placeholder: '75001' })}
            {field('library.general.city', 'addrCity', { placeholder: 'Paris' })}
          </div>
          <CountrySelect
            label={t('library.general.country')}
            value={form.addrCountry}
            required
            error={countryError ?? undefined}
            placeholder={t('library.general.countryPlaceholder')}
            onChange={(code) => {
              setForm((current) => {
                const next = applyCountryChange(
                  { country: current.addrCountry, timezone: current.timezone },
                  code,
                );
                return { ...current, addrCountry: next.country, timezone: next.timezone };
              });
              if (countryError) setCountryError(null);
              if (timezoneError) setTimezoneError(null);
            }}
          />
          <LibraryTimezoneControl
            country={form.addrCountry}
            timezone={form.timezone}
            error={timezoneError ?? undefined}
            onTimezoneChange={(timezone) => {
              setForm((current) => ({ ...current, timezone }));
              if (timezoneError) setTimezoneError(null);
            }}
          />
        </div>

        {/* Email */}
        {field('library.general.email', 'email', { type: 'email', placeholder: 'contact@bibliotheque.fr' })}

        {/* Phones */}
        <div>
          <div className="flex items-center justify-between mb-2">
            <label className={formLabelClass({ marginBottom: false, inline: true })}>
              {t('library.general.phones')}
            </label>
            <button
              onClick={addPhone}
              className="flex items-center gap-1 text-xs text-indigo-600 dark:text-indigo-400 hover:underline"
            >
              <Plus className="h-3 w-3" />
              {t('library.general.addPhone')}
            </button>
          </div>
          {form.phones.length === 0 && (
            <p className="text-sm text-gray-400 dark:text-gray-600 italic">
              {t('library.general.noPhones')}
            </p>
          )}
          <div className="space-y-2">
            {form.phones.map((phone, i) => (
              <div key={i} className="flex items-center gap-2">
                <Phone className="h-4 w-4 text-gray-400 flex-shrink-0" />
                <Input
                  type="tel"
                  value={phone}
                  onChange={(e) => setPhone(i, e.target.value)}
                  placeholder="+33 1 23 45 67 89"
                  className="flex-1"
                />
                <button
                  onClick={() => removePhone(i)}
                  className="p-1.5 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
            ))}
          </div>
        </div>

        <Button onClick={handleSave} isLoading={saving} disabled={saving} className="w-full">
          {t('common.save')}
        </Button>
      </div>
    </Card>
  );
}

// ─── Hours Tab ────────────────────────────────────────────────────────────────

function HoursTab() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const scheduleQuery = useScheduleAdminQuery();
  const periods = scheduleQuery.data?.periods ?? [];
  const slots = scheduleQuery.data?.slots ?? {};
  const closures = scheduleQuery.data?.closures ?? [];
  const loading = scheduleQuery.isLoading;
  const error = scheduleQuery.isError ? t('errors.generic') : null;

  const dayNames = DAYS_OF_WEEK.map((d) => t(`library.hours.days.${d}`));

  const [expandedPeriods, setExpandedPeriods] = useState<Set<string>>(new Set());

  // Period modal
  const [periodModal, setPeriodModal] = useState<{ open: boolean; editing?: SchedulePeriod }>({
    open: false,
  });
  const [periodForm, setPeriodForm] = useState<CreateSchedulePeriod & { notes: string }>({
    name: '',
    startDate: '',
    endDate: '',
    notes: '',
  });
  const [savingPeriod, setSavingPeriod] = useState(false);
  const [periodError, setPeriodError] = useState<string | null>(null);

  // Slot modal
  const [slotModal, setSlotModal] = useState<{ open: boolean; periodId?: string }>({
    open: false,
  });
  const [slotForm, setSlotForm] = useState<CreateScheduleSlot>({
    dayOfWeek: 0,
    openTime: '09:00',
    closeTime: '17:00',
  });
  const [savingSlot, setSavingSlot] = useState(false);
  const [slotError, setSlotError] = useState<string | null>(null);

  // Closure modal
  const [closureModal, setClosureModal] = useState(false);
  const [closureForm, setClosureForm] = useState<CreateScheduleClosure>({
    closureDate: '',
    reason: '',
  });
  const [savingClosure, setSavingClosure] = useState(false);
  const [closureError, setClosureError] = useState<string | null>(null);

  // Delete in-progress tracking
  const [deletingPeriod, setDeletingPeriod] = useState<string | null>(null);
  const [deletingSlot, setDeletingSlot] = useState<string | null>(null);
  const [deletingClosure, setDeletingClosure] = useState<string | null>(null);

  useEffect(() => {
    if (periods.length > 0 && expandedPeriods.size === 0) {
      setExpandedPeriods(new Set(periods.map((p) => p.id)));
    }
  }, [periods, expandedPeriods.size]);

  const refreshScheduleAdmin = async () => {
    await invalidateScheduleQueries(queryClient);
  };

  // ── Period handlers ──────────────────────────────────────────────────────

  const openAddPeriod = () => {
    setPeriodForm({ name: '', startDate: '', endDate: '', notes: '' });
    setPeriodError(null);
    setPeriodModal({ open: true });
  };

  const openEditPeriod = (period: SchedulePeriod) => {
    setPeriodForm({
      name: period.name,
      startDate: period.startDate,
      endDate: period.endDate,
      notes: period.notes ?? '',
    });
    setPeriodError(null);
    setPeriodModal({ open: true, editing: period });
  };

  const savePeriod = async () => {
    if (!periodForm.name || !periodForm.startDate || !periodForm.endDate) return;
    setSavingPeriod(true);
    setPeriodError(null);
    try {
      const payload: UpdateSchedulePeriod = {
        name: periodForm.name,
        startDate: periodForm.startDate,
        endDate: periodForm.endDate,
        notes: periodForm.notes || null,
      };
      if (periodModal.editing) {
        await api.updateSchedulePeriod(periodModal.editing.id, payload);
      } else {
        const created = await api.createSchedulePeriod({
          name: periodForm.name,
          startDate: periodForm.startDate,
          endDate: periodForm.endDate,
          notes: periodForm.notes || null,
        });
        setExpandedPeriods((prev) => new Set([...prev, created.id]));
      }
      await refreshScheduleAdmin();
      setPeriodModal({ open: false });
    } catch {
      setPeriodError(t('errors.generic'));
    } finally {
      setSavingPeriod(false);
    }
  };

  const deletePeriod = async (id: string) => {
    setDeletingPeriod(id);
    try {
      await api.deleteSchedulePeriod(id);
      await refreshScheduleAdmin();
    } catch {
      // silently ignore
    } finally {
      setDeletingPeriod(null);
    }
  };

  // ── Slot handlers ────────────────────────────────────────────────────────

  const openAddSlot = (periodId: string, dayOfWeek: number) => {
    setSlotForm({ dayOfWeek: dayOfWeek, openTime: '09:00', closeTime: '17:00' });
    setSlotError(null);
    setSlotModal({ open: true, periodId });
  };

  const saveSlot = async () => {
    if (!slotModal.periodId) return;
    setSavingSlot(true);
    setSlotError(null);
    try {
      await api.createScheduleSlot(slotModal.periodId, slotForm);
      await refreshScheduleAdmin();
      setSlotModal({ open: false });
    } catch {
      setSlotError(t('errors.generic'));
    } finally {
      setSavingSlot(false);
    }
  };

  const deleteSlot = async (slotId: string, _periodId: string) => {
    setDeletingSlot(slotId);
    try {
      await api.deleteScheduleSlot(slotId);
      await refreshScheduleAdmin();
    } catch {
      // silently ignore
    } finally {
      setDeletingSlot(null);
    }
  };

  // ── Closure handlers ─────────────────────────────────────────────────────

  const openAddClosure = () => {
    setClosureForm({ closureDate: '', reason: '' });
    setClosureError(null);
    setClosureModal(true);
  };

  const saveClosure = async () => {
    if (!closureForm.closureDate) return;
    setSavingClosure(true);
    setClosureError(null);
    try {
      await api.createScheduleClosure({
        closureDate: closureForm.closureDate,
        reason: (closureForm.reason as string) || null,
      });
      await refreshScheduleAdmin();
      setClosureModal(false);
    } catch {
      setClosureError(t('errors.generic'));
    } finally {
      setSavingClosure(false);
    }
  };

  const deleteClosure = async (id: string) => {
    setDeletingClosure(id);
    try {
      await api.deleteScheduleClosure(id);
      await refreshScheduleAdmin();
    } catch {
      // silently ignore
    } finally {
      setDeletingClosure(null);
    }
  };

  const togglePeriod = (id: string) => {
    setExpandedPeriods((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  if (loading) {
    return (
      <div className="flex justify-center py-12">
        <div className="h-8 w-8 border-2 border-amber-600 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (error) {
    return <div className="text-center py-8 text-red-600 dark:text-red-400">{error}</div>;
  }

  return (
    <div className="space-y-8">
      {/* ── Periods ──────────────────────────────────────────────────────── */}
      <section>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-base font-semibold text-gray-900 dark:text-white">
            {t('library.hours.periods')}
          </h2>
          <Button size="sm" onClick={openAddPeriod}>
            <Plus className="h-4 w-4" />
            {t('library.hours.addPeriod')}
          </Button>
        </div>

        {periods.length === 0 ? (
          <Card padding="none">
            <div className="py-10 text-center text-gray-400 dark:text-gray-600">
              <Clock className="h-10 w-10 mx-auto mb-2 opacity-40" />
              <p className="text-sm">{t('library.hours.noPeriods')}</p>
            </div>
          </Card>
        ) : (
          <div className="space-y-3">
            {periods.map((period) => {
              const isExpanded = expandedPeriods.has(period.id);
              const periodSlots = slots[period.id] ?? [];

              return (
                <Card key={period.id} padding="none">
                  {/* Period header */}
                  <div
                    className="flex items-center gap-3 px-4 py-3 cursor-pointer select-none"
                    onClick={() => togglePeriod(period.id)}
                  >
                    {isExpanded ? (
                      <ChevronDown className="h-4 w-4 text-gray-400 flex-shrink-0" />
                    ) : (
                      <ChevronRight className="h-4 w-4 text-gray-400 flex-shrink-0" />
                    )}
                    <div className="flex-1 min-w-0">
                      <p className="font-medium text-gray-900 dark:text-white text-sm">
                        {period.name}
                      </p>
                      <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                        {period.startDate} → {period.endDate}
                        {period.notes ? ` · ${period.notes}` : ''}
                      </p>
                    </div>
                    <div
                      className="flex gap-1 flex-shrink-0"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <button
                        onClick={() => openEditPeriod(period)}
                        className="p-1.5 rounded-lg hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 transition-colors"
                        title={t('common.edit')}
                      >
                        <Pencil className="h-4 w-4" />
                      </button>
                      <button
                        onClick={() => deletePeriod(period.id)}
                        disabled={deletingPeriod === period.id}
                        className="p-1.5 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors disabled:opacity-50"
                        title={t('common.delete')}
                      >
                        <Trash2 className="h-4 w-4" />
                      </button>
                    </div>
                  </div>

                  {/* Expanded weekly grid */}
                  {isExpanded && (
                    <div className="border-t border-gray-100 dark:border-gray-800 px-4 py-4">
                      <div className="space-y-2">
                        {DAYS_OF_WEEK.map((day) => {
                          const daySlots = periodSlots
                            .filter((s) => s.dayOfWeek === day)
                            .sort((a, b) => a.openTime.localeCompare(b.openTime));

                          return (
                            <div key={day} className="flex items-center gap-3 min-h-[2.25rem]">
                              <span className="w-24 text-xs font-medium text-gray-500 dark:text-gray-400 flex-shrink-0">
                                {dayNames[day]}
                              </span>
                              <div className="flex flex-wrap gap-2 flex-1 items-center">
                                {daySlots.length === 0 && (
                                  <span className="text-xs text-gray-300 dark:text-gray-700 italic">
                                    {t('library.hours.closed')}
                                  </span>
                                )}
                                {daySlots.map((slot) => (
                                  <span
                                    key={slot.id}
                                    className="inline-flex items-center gap-1 bg-indigo-50 dark:bg-indigo-900/20 text-indigo-700 dark:text-indigo-300 rounded-full px-2.5 py-1 text-xs"
                                  >
                                    {slot.openTime} – {slot.closeTime}
                                    <button
                                      onClick={() => deleteSlot(slot.id, period.id)}
                                      disabled={deletingSlot === slot.id}
                                      className="ml-0.5 rounded-full p-0.5 hover:bg-indigo-200 dark:hover:bg-indigo-700 transition-colors disabled:opacity-50"
                                    >
                                      <X className="h-3 w-3" />
                                    </button>
                                  </span>
                                ))}
                                <button
                                  onClick={() => openAddSlot(period.id, day)}
                                  className="inline-flex items-center gap-1 text-xs text-gray-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-full px-2 py-1 transition-colors"
                                >
                                  <Plus className="h-3 w-3" />
                                </button>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </Card>
              );
            })}
          </div>
        )}
      </section>

      {/* ── Closures ─────────────────────────────────────────────────────── */}
      <section>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-base font-semibold text-gray-900 dark:text-white">
            {t('library.hours.closures')}
          </h2>
          <Button size="sm" variant="secondary" onClick={openAddClosure}>
            <Plus className="h-4 w-4" />
            {t('library.hours.addClosure')}
          </Button>
        </div>

        <Card padding="none">
          {closures.length === 0 ? (
            <div className="py-10 text-center text-gray-400 dark:text-gray-600">
              <CalendarOff className="h-10 w-10 mx-auto mb-2 opacity-40" />
              <p className="text-sm">{t('library.hours.noClosures')}</p>
            </div>
          ) : (
            <div className="divide-y divide-gray-100 dark:divide-gray-800">
              {closures.map((closure) => (
                <div key={closure.id} className="flex items-center gap-3 px-4 py-3">
                  <div className="flex-1 min-w-0">
                    <p className="text-sm font-medium text-gray-900 dark:text-white">
                      {closure.closureDate}
                    </p>
                    {closure.reason && (
                      <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                        {closure.reason}
                      </p>
                    )}
                  </div>
                  <button
                    onClick={() => deleteClosure(closure.id)}
                    disabled={deletingClosure === closure.id}
                    className="p-1.5 rounded-lg hover:bg-red-50 dark:hover:bg-red-900/20 text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors disabled:opacity-50"
                  >
                    <Trash2 className="h-4 w-4" />
                  </button>
                </div>
              ))}
            </div>
          )}
        </Card>
      </section>

      {/* ── Period Modal ─────────────────────────────────────────────────── */}
      <Modal
        isOpen={periodModal.open}
        onClose={() => setPeriodModal({ open: false })}
        title={periodModal.editing ? t('library.hours.editPeriod') : t('library.hours.addPeriod')}
      >
        <div className="space-y-4">
          {periodError && (
            <p className="text-sm text-red-600 dark:text-red-400">{periodError}</p>
          )}
          <div>
            <label className={formLabelClass()}>
              {t('library.hours.periodName')} *
            </label>
            <Input
              value={periodForm.name}
              onChange={(e) => setPeriodForm((f) => ({ ...f, name: e.target.value }))}
              placeholder={t('library.hours.periodName')}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={formLabelClass()}>
                {t('library.hours.startDate')} *
              </label>
              <Input
                type="date"
                value={periodForm.startDate}
                onChange={(e) => setPeriodForm((f) => ({ ...f, startDate: e.target.value }))}
              />
            </div>
            <div>
              <label className={formLabelClass()}>
                {t('library.hours.endDate')} *
              </label>
              <Input
                type="date"
                value={periodForm.endDate}
                onChange={(e) => setPeriodForm((f) => ({ ...f, endDate: e.target.value }))}
              />
            </div>
          </div>
          <div>
            <label className={formLabelClass()}>
              {t('library.hours.notes')}
            </label>
            <Input
              value={periodForm.notes}
              onChange={(e) => setPeriodForm((f) => ({ ...f, notes: e.target.value }))}
              placeholder={t('library.hours.notes')}
            />
          </div>
          <div className="flex gap-3 justify-end pt-1">
            <Button variant="secondary" onClick={() => setPeriodModal({ open: false })}>
              {t('common.cancel')}
            </Button>
            <Button
              onClick={savePeriod}
              isLoading={savingPeriod}
              disabled={savingPeriod || !periodForm.name || !periodForm.startDate || !periodForm.endDate}
            >
              {t('common.save')}
            </Button>
          </div>
        </div>
      </Modal>

      {/* ── Slot Modal ───────────────────────────────────────────────────── */}
      <Modal
        isOpen={slotModal.open}
        onClose={() => setSlotModal({ open: false })}
        title={t('library.hours.addSlot')}
      >
        <div className="space-y-4">
          {slotError && <p className="text-sm text-red-600 dark:text-red-400">{slotError}</p>}
          <div>
            <label className={formLabelClass()}>
              {t('library.hours.day')}
            </label>
            <select
              value={slotForm.dayOfWeek}
              onChange={(e) =>
                setSlotForm((f) => ({ ...f, dayOfWeek: Number(e.target.value) }))
              }
              className={formControlClass({ className: 'w-full' })}
            >
              {DAYS_OF_WEEK.map((d) => (
                <option key={d} value={d}>
                  {dayNames[d]}
                </option>
              ))}
            </select>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className={formLabelClass()}>
                {t('library.hours.openTime')}
              </label>
              <Input
                type="time"
                value={slotForm.openTime}
                onChange={(e) => setSlotForm((f) => ({ ...f, openTime: e.target.value }))}
              />
            </div>
            <div>
              <label className={formLabelClass()}>
                {t('library.hours.closeTime')}
              </label>
              <Input
                type="time"
                value={slotForm.closeTime}
                onChange={(e) => setSlotForm((f) => ({ ...f, closeTime: e.target.value }))}
              />
            </div>
          </div>
          <div className="flex gap-3 justify-end pt-1">
            <Button variant="secondary" onClick={() => setSlotModal({ open: false })}>
              {t('common.cancel')}
            </Button>
            <Button onClick={saveSlot} isLoading={savingSlot} disabled={savingSlot}>
              {t('common.add')}
            </Button>
          </div>
        </div>
      </Modal>

      {/* ── Closure Modal ────────────────────────────────────────────────── */}
      <Modal
        isOpen={closureModal}
        onClose={() => setClosureModal(false)}
        title={t('library.hours.addClosure')}
      >
        <div className="space-y-4">
          {closureError && (
            <p className="text-sm text-red-600 dark:text-red-400">{closureError}</p>
          )}
          <div>
            <label className={formLabelClass()}>
              {t('library.hours.closureDate')} *
            </label>
            <Input
              type="date"
              value={closureForm.closureDate}
              onChange={(e) =>
                setClosureForm((f) => ({ ...f, closureDate: e.target.value }))
              }
            />
          </div>
          <div>
            <label className={formLabelClass()}>
              {t('library.hours.closureReason')}
            </label>
            <Input
              value={closureForm.reason ?? ''}
              onChange={(e) => setClosureForm((f) => ({ ...f, reason: e.target.value }))}
              placeholder={t('library.hours.closureReasonPlaceholder')}
            />
          </div>
          <div className="flex gap-3 justify-end pt-1">
            <Button variant="secondary" onClick={() => setClosureModal(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              onClick={saveClosure}
              isLoading={savingClosure}
              disabled={savingClosure || !closureForm.closureDate}
            >
              {t('common.add')}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}

// ─── Panels (used in Settings › Library tab) ──────────────────────────────────

/** Library identity, address, contact, opening hours — embeddable in Settings. */
export function LibrarySettingsPanel() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<'general' | 'hours'>('general');

  const tabs = [
    { id: 'general' as const, label: t('library.tabs.general'), icon: Building2 },
    { id: 'hours' as const, label: t('library.tabs.hours'), icon: Clock },
  ];

  return (
    <div className="space-y-6">
      <SectionTabBar
        ariaLabel={t('library.title')}
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        tabs={tabs.map(({ id, label, icon }) => ({ id, label, icon }))}
      />

      {activeTab === 'general' && <GeneralTab />}
      {activeTab === 'hours' && <HoursTab />}
    </div>
  );
}
