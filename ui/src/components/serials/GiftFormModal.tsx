import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Input, Modal, BarcodeScanField } from '@/components/common';
import Button from '@/components/common/Button';
import CallNumberField from '@/components/specimen/CallNumberField';
import Typeahead from '@/components/common/Typeahead';
import api from '@/services/api';
import type { CreateSerialGift, Site } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';

type GiftFormValues = {
  biblioId: string;
  biblioTitle: string;
  newBiblioMode: boolean;
  newBiblioTitle: string;
  newBiblioIssn: string;
  siteId: string;
  volumeLabel: string;
  coverDate: string;
  barcode: string;
  callNumber: string;
  internalNote: string;
};

const emptyValues = (): GiftFormValues => ({
  biblioId: '',
  biblioTitle: '',
  newBiblioMode: false,
  newBiblioTitle: '',
  newBiblioIssn: '',
  siteId: '',
  volumeLabel: '',
  coverDate: '',
  barcode: '',
  callNumber: '',
  internalNote: '',
});

type GiftFormModalProps = {
  isOpen: boolean;
  isSaving: boolean;
  error: unknown;
  sites: Site[];
  initialBiblioId?: string;
  initialBiblioTitle?: string;
  onClose: () => void;
  onSubmit: (body: CreateSerialGift) => void;
};

export default function GiftFormModal({
  isOpen,
  isSaving,
  error,
  sites,
  initialBiblioId,
  initialBiblioTitle,
  onClose,
  onSubmit,
}: GiftFormModalProps) {
  const { t } = useTranslation();
  const [values, setValues] = useState<GiftFormValues>(emptyValues);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [biblioQuery, setBiblioQuery] = useState('');
  const [debouncedBiblio, setDebouncedBiblio] = useState('');
  const [creatingBiblio, setCreatingBiblio] = useState(false);
  const [localError, setLocalError] = useState<unknown>(null);

  useEffect(() => {
    if (!isOpen) return;
    const next = emptyValues();
    const defaultSite = sites.find((s) => s.default)?.id ?? sites[0]?.id ?? '';
    next.siteId = defaultSite ? String(defaultSite) : '';
    if (initialBiblioId) {
      next.biblioId = initialBiblioId;
      next.biblioTitle = initialBiblioTitle?.trim() || initialBiblioId;
    }
    setValues(next);
    setFieldErrors({});
    setBiblioQuery('');
    setLocalError(null);
  }, [isOpen, initialBiblioId, initialBiblioTitle, sites]);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedBiblio(biblioQuery), 300);
    return () => window.clearTimeout(timer);
  }, [biblioQuery]);

  const biblioSearch = useQuery({
    queryKey: ['serial-periodicals', debouncedBiblio],
    queryFn: () =>
      api.getBiblios({
        title: debouncedBiblio,
        mediaType: 'periodic',
        perPage: 8,
        page: 1,
        includeWithoutActiveItems: true,
      }),
    enabled: isOpen && debouncedBiblio.trim().length >= 2 && !values.biblioId && !values.newBiblioMode,
  });

  const patch = (next: Partial<GiftFormValues>) => setValues((current) => ({ ...current, ...next }));

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    const errors: Record<string, string> = {};
    if (!values.biblioId && !values.newBiblioMode) errors.biblioId = 'required';
    if (values.newBiblioMode && !values.newBiblioTitle.trim()) errors.newBiblioTitle = 'required';
    if (!values.siteId) errors.siteId = 'required';
    if (!values.volumeLabel.trim()) errors.volumeLabel = 'required';
    setFieldErrors(errors);
    if (Object.keys(errors).length > 0) return;

    setLocalError(null);
    let biblioId = values.biblioId;
    if (values.newBiblioMode) {
      setCreatingBiblio(true);
      try {
        const issn = values.newBiblioIssn.trim();
        const created = await api.createBiblio({
          title: values.newBiblioTitle.trim(),
          mediaType: 'periodic',
          ...(issn ? { issn } : {}),
        });
        const id = created.biblio?.id;
        if (!id) throw new Error('Created periodical has no id');
        biblioId = String(id);
      } catch (createError) {
        setLocalError(createError);
        setCreatingBiblio(false);
        return;
      }
      setCreatingBiblio(false);
    }

    const body: CreateSerialGift = {
      biblioId,
      siteId: values.siteId,
      volumeLabel: values.volumeLabel.trim(),
      coverDate: values.coverDate.trim() || null,
      barcode: values.barcode.trim() || null,
      callNumber: values.callNumber.trim() || null,
      internalNote: values.internalNote.trim() || null,
    };
    onSubmit(body);
  };

  const busy = isSaving || creatingBiblio;
  const submitError = localError ?? error;
  const submitMessage = submitError ? getApiErrorMessage(submitError, t) : null;
  const fieldError = (key: string) =>
    fieldErrors[key] ? t('serials.form.required') : undefined;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('serials.gifts.formTitle')}
      size="lg"
      footer={
        <div className="flex flex-wrap justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={busy}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" form="serial-gift-form" isLoading={busy}>
            {t('serials.gifts.save')}
          </Button>
        </div>
      }
    >
      <form id="serial-gift-form" className="space-y-3" onSubmit={handleSubmit}>
        {submitMessage ? (
          <p className="text-sm text-red-600 dark:text-red-400">{submitMessage}</p>
        ) : null}
        <Typeahead
          label={t('serials.form.periodical')}
          value={values.biblioId ? values.biblioTitle : biblioQuery}
          onChange={(value) => {
            patch({
              biblioId: '',
              biblioTitle: '',
              newBiblioMode: false,
              newBiblioTitle: '',
              newBiblioIssn: '',
            });
            setBiblioQuery(value);
          }}
          items={!values.biblioId && !values.newBiblioMode && biblioQuery.trim().length >= 2 ? (biblioSearch.data?.items ?? []) : []}
          getItemId={(biblio) => biblio.id}
          onSelect={(biblio) =>
            patch({
              biblioId: biblio.id,
              biblioTitle: biblio.title?.trim() || biblio.id,
              newBiblioMode: false,
            })
          }
          renderItem={(biblio) =>
            biblio.issn ? `${biblio.title || biblio.id} · ISSN ${biblio.issn}` : biblio.title || biblio.id
          }
          placeholder={t('serials.form.periodicalSearch')}
          disabled={values.newBiblioMode}
          loading={biblioSearch.isFetching}
          selectedId={values.biblioId || null}
          error={fieldError('biblioId')}
          variant="dropdown"
        />
        {!values.biblioId && !values.newBiblioMode && biblioQuery.trim().length >= 2 && !biblioSearch.isFetching ? (
          <div>
            <Button
              type="button"
              variant="secondary"
              onClick={() =>
                patch({
                  newBiblioMode: true,
                  newBiblioTitle: biblioQuery.trim(),
                  newBiblioIssn: '',
                  biblioId: '',
                  biblioTitle: '',
                })
              }
            >
              {t('serials.form.createPeriodical', { title: biblioQuery.trim() })}
            </Button>
          </div>
        ) : null}
        {values.newBiblioMode ? (
          <div className="grid gap-3 rounded-lg border border-amber-200 bg-amber-50/60 p-3 dark:border-amber-900/60 dark:bg-amber-950/30 sm:grid-cols-2">
            <Input
              id="gift-new-title"
              label={t('serials.form.newPeriodicalTitle')}
              value={values.newBiblioTitle}
              onChange={(event) => patch({ newBiblioTitle: event.target.value })}
              error={fieldError('newBiblioTitle')}
              autoComplete="off"
            />
            <Input
              id="gift-new-issn"
              label={t('serials.form.newPeriodicalIssn')}
              value={values.newBiblioIssn}
              onChange={(event) => patch({ newBiblioIssn: event.target.value })}
              autoComplete="off"
            />
            <div className="sm:col-span-2">
              <Button
                type="button"
                variant="secondary"
                onClick={() =>
                  patch({
                    newBiblioMode: false,
                    newBiblioTitle: '',
                    newBiblioIssn: '',
                  })
                }
              >
                {t('serials.form.cancelNewPeriodical')}
              </Button>
            </div>
          </div>
        ) : null}

        <div className="grid gap-3 sm:grid-cols-2">
          <Input
            id="gift-volume"
            label={t('serials.gifts.volumeLabel')}
            value={values.volumeLabel}
            onChange={(event) => patch({ volumeLabel: event.target.value })}
            error={fieldError('volumeLabel')}
            placeholder={t('serials.gifts.volumePlaceholder')}
            autoComplete="off"
          />
          <Input
            id="gift-cover-date"
            type="date"
            label={t('serials.gifts.coverDate')}
            value={values.coverDate}
            onChange={(event) => patch({ coverDate: event.target.value })}
          />
        </div>

        <div>
          <label htmlFor="gift-site" className={formLabelClass()}>
            {t('serials.form.site')}
          </label>
          <select
            id="gift-site"
            className={formControlClass()}
            value={values.siteId}
            onChange={(event) => patch({ siteId: event.target.value })}
          >
            <option value="">{t('items.selectSite')}</option>
            {sites.map((site) => (
              <option key={site.id} value={String(site.id)}>
                {site.name || site.id}
                {site.default ? ` (${t('importMarc.default')})` : ''}
              </option>
            ))}
          </select>
          {fieldError('siteId') ? (
            <p className="mt-1 text-sm text-red-600 dark:text-red-400">{fieldError('siteId')}</p>
          ) : null}
        </div>

        <div>
          <BarcodeScanField
            label={t('items.specimenBarcode')}
            value={values.barcode}
            onChange={(e) => patch({ barcode: e.target.value })}
            scannerTitle={t('items.specimenBarcode')}
            onCameraScan={(code) => patch({ barcode: code })}
          />
          <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{t('serials.gifts.barcodeHint')}</p>
        </div>
        <CallNumberField
          label={t('items.callNumber')}
          value={values.callNumber}
          onChange={(v) => patch({ callNumber: v })}
          suggestions={[]}
          placeholder={t('items.callNumber')}
        />
        <Input
          id="gift-note"
          label={t('serials.gifts.internalNote')}
          value={values.internalNote}
          onChange={(event) => patch({ internalNote: event.target.value })}
          autoComplete="off"
        />
      </form>
    </Modal>
  );
}
