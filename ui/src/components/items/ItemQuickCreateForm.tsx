import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AlertCircle } from 'lucide-react';
import { Input, BarcodeScanField } from '@/components/common';
import CallNumberField from '@/components/specimen/CallNumberField';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import { validateCallNumber, type CallNumberOption } from '@/utils/callNumber';
import { getApiErrorMessage } from '@/utils/apiError';
import api from '@/services/api';
import type { Site } from '@/types';

export type ItemQuickCreateValues = {
  barcode: string;
  callNumber: string;
  siteId: string;
  volumeDesignation: string;
  borrowable: 'true' | 'false';
};

export const emptyItemQuickCreateValues = (): ItemQuickCreateValues => ({
  barcode: '',
  callNumber: '',
  siteId: '',
  volumeDesignation: '',
  borrowable: 'true',
});

function defaultSiteIdFromList(sites: Site[]): string {
  return sites.find((s) => s.default)?.id ?? sites[0]?.id ?? '';
}

type FieldsProps = {
  values: ItemQuickCreateValues;
  onChange: (next: ItemQuickCreateValues) => void;
  callNumberSuggestions: CallNumberOption[];
  sites: Site[];
  /** Show volume designation + borrowable (detail modal). */
  showExtendedFields?: boolean;
  /** Single row for create-with-notice (barcode | call number | site). */
  layout?: 'stack' | 'row';
  error?: string | null;
  onClearError?: () => void;
};

/** Presentational fields shared by create-with-notice and attach-to-biblio. */
export function ItemQuickCreateFields({
  values,
  onChange,
  callNumberSuggestions,
  sites,
  showExtendedFields = false,
  layout = 'stack',
  error = null,
  onClearError,
}: FieldsProps) {
  const { t } = useTranslation();
  const set = (patch: Partial<ItemQuickCreateValues>) => {
    onClearError?.();
    onChange({ ...values, ...patch });
  };

  const siteSelect = (
    <div className="min-w-0">
      <label className={formLabelClass()}>{t('items.site')}</label>
      <select
        value={values.siteId}
        onChange={(e) => set({ siteId: e.target.value })}
        className={formControlClass({ className: 'w-full' })}
      >
        <option value="">{t('items.selectSite')}</option>
        {sites.map((src) => (
          <option key={src.id} value={src.id}>
            {src.name || src.id}
            {src.default ? ` (${t('importMarc.default')})` : ''}
          </option>
        ))}
      </select>
    </div>
  );

  const errorBanner = error ? (
    <div className="flex items-center gap-2 p-2.5 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-400 text-sm">
      <AlertCircle className="h-4 w-4 flex-shrink-0" />
      {error}
    </div>
  ) : null;

  if (layout === 'row' && !showExtendedFields) {
    return (
      <div className="space-y-2">
        {errorBanner}
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 items-end">
          <BarcodeScanField
            label={t('items.specimenBarcode')}
            value={values.barcode}
            onChange={(e) => set({ barcode: e.target.value })}
            scannerTitle={t('items.specimenBarcode')}
            onCameraScan={(code) => set({ barcode: code })}
          />
          <CallNumberField
            label={t('items.callNumber')}
            value={values.callNumber}
            onChange={(v) => set({ callNumber: v })}
            suggestions={callNumberSuggestions}
            placeholder={t('items.callNumber')}
          />
          {siteSelect}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <BarcodeScanField
        label={t('items.specimenBarcode')}
        value={values.barcode}
        onChange={(e) => set({ barcode: e.target.value })}
        scannerTitle={t('items.specimenBarcode')}
        onCameraScan={(code) => set({ barcode: code })}
      />
      {errorBanner}
      <CallNumberField
        label={t('items.callNumber')}
        value={values.callNumber}
        onChange={(v) => set({ callNumber: v })}
        suggestions={callNumberSuggestions}
        placeholder={t('items.callNumber')}
      />
      {siteSelect}
      {showExtendedFields ? (
        <>
          <Input
            label={t('items.volumeDesignation')}
            value={values.volumeDesignation}
            onChange={(e) => set({ volumeDesignation: e.target.value })}
            placeholder={t('items.volumeDesignationPlaceholder')}
          />
          <div>
            <label className={formLabelClass()}>{t('items.borrowable')}</label>
            <select
              value={values.borrowable}
              onChange={(e) => set({ borrowable: e.target.value as 'true' | 'false' })}
              className={formControlClass({ className: 'w-full' })}
            >
              <option value="true">{t('items.borrowableYes')}</option>
              <option value="false">{t('items.borrowableNo')}</option>
            </select>
          </div>
        </>
      ) : null}
    </div>
  );
}

type DraftPanelProps = {
  values: ItemQuickCreateValues;
  onChange: (next: ItemQuickCreateValues) => void;
  callNumberSuggestions: CallNumberOption[];
  error?: string | null;
  onClearError?: () => void;
};

/** Loads sites and renders compact fields for create-with-notice. */
export function ItemQuickCreateDraftPanel({
  values,
  onChange,
  callNumberSuggestions,
  error = null,
  onClearError,
}: DraftPanelProps) {
  const [sites, setSites] = useState<Site[]>([]);

  useEffect(() => {
    api
      .getSites(false)
      .then(setSites)
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (sites.length === 0 || values.siteId.trim() !== '') return;
    const id = defaultSiteIdFromList(sites);
    if (!id) return;
    onChange({ ...values, siteId: id });
    // Only seed default site once sites load and siteId is still empty.
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentional one-shot seed
  }, [sites, values.siteId]);

  return (
    <ItemQuickCreateFields
      values={values}
      onChange={onChange}
      callNumberSuggestions={callNumberSuggestions}
      sites={sites}
      layout="row"
      showExtendedFields={false}
      error={error}
      onClearError={onClearError}
    />
  );
}

type AttachProps = {
  formId: string;
  biblioId: string;
  callNumberSuggestions: CallNumberOption[];
  onLoadingChange: (loading: boolean) => void;
  onSuccess: () => void;
};

/** Modal form: POST /biblios/{id}/items */
export default function ItemQuickCreateForm({
  formId,
  biblioId,
  callNumberSuggestions,
  onLoadingChange,
  onSuccess,
}: AttachProps) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const [sites, setSites] = useState<Site[]>([]);
  const [values, setValues] = useState<ItemQuickCreateValues>(emptyItemQuickCreateValues);

  useEffect(() => {
    api
      .getSites(false)
      .then((list) => {
        setSites(list);
        setValues((prev) => {
          if (prev.siteId.trim() !== '') return prev;
          const id = defaultSiteIdFromList(list);
          return id ? { ...prev, siteId: id } : prev;
        });
      })
      .catch(() => {});
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (values.barcode.trim() === '') {
      setError(t('items.specimenBarcodeRequired'));
      return;
    }
    if (!validateCallNumber(values.callNumber)) return;
    onLoadingChange(true);
    setError(null);
    try {
      await api.createItem(biblioId, {
        barcode: values.barcode.trim(),
        callNumber: values.callNumber || undefined,
        volumeDesignation: values.volumeDesignation || undefined,
        borrowable: values.borrowable === 'true',
        siteId: values.siteId || undefined,
      });
      onSuccess();
    } catch (err) {
      setError(getApiErrorMessage(err, t));
    } finally {
      onLoadingChange(false);
    }
  };

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4">
      <ItemQuickCreateFields
        values={values}
        onChange={setValues}
        callNumberSuggestions={callNumberSuggestions}
        sites={sites}
        showExtendedFields
        error={error}
        onClearError={() => setError(null)}
      />
    </form>
  );
}
