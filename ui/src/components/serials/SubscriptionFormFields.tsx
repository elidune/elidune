import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import CollapsibleFormSection from '@/components/common/CollapsibleFormSection';
import Input from '@/components/common/Input';
import type { PredictedSerialIssue, SerialFrequency, SerialNumberingPattern, Site } from '@/types';
import type { AcquisitionFund, AcquisitionVendor, BiblioShortStaff } from '@/types';
import { formChoiceControlClass, formChoiceLabelClass, formControlClass, formLabelClass } from '@/utils/formControl';
import {
  frequencyGracePlaceholder,
  formatIsoDate,
  frequencyLabel,
  type SubscriptionFormValues,
} from '@/utils/serialSubscriptionDisplay';

const FIELD_ERROR = {
  required: 'serials.form.required',
  invalidAmount: 'serials.form.invalidAmount',
  invalidNumber: 'serials.form.invalidNumber',
} as const;

export interface SubscriptionFormFieldsProps {
  values: SubscriptionFormValues;
  onChange: (patch: Partial<SubscriptionFormValues>) => void;
  errors: Record<string, 'required' | 'invalidAmount' | 'invalidNumber'>;
  frequencies: SerialFrequency[];
  patterns: SerialNumberingPattern[];
  sites: Site[];
  vendors: AcquisitionVendor[];
  funds: AcquisitionFund[];
  biblioQuery: string;
  onBiblioQuery: (value: string) => void;
  biblioResults: BiblioShortStaff[];
  biblioSearching: boolean;
  onSelectBiblio: (id: string, title: string) => void;
  predictionLocked: boolean;
  preview: PredictedSerialIssue[];
  previewLoading: boolean;
  previewError: string | null;
  emptyHorizon: boolean;
}

export default function SubscriptionFormFields({
  values,
  onChange,
  errors,
  frequencies,
  patterns,
  sites,
  vendors,
  funds,
  biblioQuery,
  onBiblioQuery,
  biblioResults,
  biblioSearching,
  onSelectBiblio,
  predictionLocked,
  preview,
  previewLoading,
  previewError,
  emptyHorizon,
}: SubscriptionFormFieldsProps) {
  const { t, i18n } = useTranslation();
  const previewErrorId = useId();
  const [advancedOpen, setAdvancedOpen] = useState(true);
  const pattern = patterns.find((row) => row.id === values.numberingPatternId);
  const frequency = frequencies.find((row) => row.id === values.frequencyId);
  const gracePlaceholder = frequencyGracePlaceholder(frequency);
  const fieldError = (key: string) => {
    const code = errors[key];
    return code ? t(FIELD_ERROR[code]) : undefined;
  };

  return (
    <div className="space-y-3">
      <Input
        id="serial-periodical-search"
        label={t('serials.form.periodical')}
        value={values.biblioId ? values.biblioTitle : biblioQuery}
        onChange={(event) => {
          onChange({ biblioId: '', biblioTitle: '' });
          onBiblioQuery(event.target.value);
        }}
        placeholder={t('serials.form.periodicalSearch')}
        error={fieldError('biblioId')}
        autoComplete="off"
        disabled={predictionLocked && Boolean(values.biblioId)}
      />
      {!values.biblioId && biblioQuery.trim().length >= 2 ? (
        <ul className="max-h-40 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700" aria-label={t('serials.form.periodical')}>
          {biblioSearching ? (
            <li className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400">{t('common.loading')}</li>
          ) : biblioResults.length === 0 ? (
            <li className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400">{t('serials.form.periodicalEmpty')}</li>
          ) : (
            biblioResults.map((biblio) => (
              <li key={biblio.id}>
                <button
                  type="button"
                  className="w-full px-3 py-2 text-left text-sm text-gray-900 hover:bg-amber-50 dark:text-gray-100 dark:hover:bg-gray-800"
                  onClick={() => onSelectBiblio(biblio.id, biblio.title?.trim() || biblio.id)}
                >
                  {biblio.title || biblio.id}
                </button>
              </li>
            ))
          )}
        </ul>
      ) : null}

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <label htmlFor="serial-site" className={formLabelClass()}>
            {t('serials.form.site')}
          </label>
          <select
            id="serial-site"
            className={formControlClass({ error: Boolean(errors.siteId) })}
            value={values.siteId}
            aria-invalid={errors.siteId ? true : undefined}
            aria-describedby={errors.siteId ? 'serial-site-error' : undefined}
            onChange={(event) => onChange({ siteId: event.target.value })}
          >
            <option value="">{t('serials.form.site')}</option>
            {sites.map((site) => (
              <option key={site.id} value={String(site.id)}>
                {site.name || site.id}
              </option>
            ))}
          </select>
          {errors.siteId ? (
            <p id="serial-site-error" className="mt-1 text-sm text-red-600 dark:text-red-400">
              {fieldError('siteId')}
            </p>
          ) : null}
        </div>
        <div>
          <label htmlFor="serial-vendor" className={formLabelClass()}>
            {t('serials.form.vendor')}
          </label>
          <select
            id="serial-vendor"
            className={formControlClass()}
            value={values.vendorId}
            onChange={(event) => onChange({ vendorId: event.target.value })}
          >
            <option value="">{t('serials.form.vendorNone')}</option>
            {vendors.map((vendor) => (
              <option key={vendor.id} value={vendor.id}>
                {vendor.name}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="serial-fund" className={formLabelClass()}>
            {t('serials.form.fund')}
          </label>
          <select
            id="serial-fund"
            className={formControlClass()}
            value={values.fundId}
            onChange={(event) => onChange({ fundId: event.target.value })}
          >
            <option value="">{t('serials.form.fundNone')}</option>
            {funds.map((fund) => (
              <option key={fund.id} value={fund.id}>
                {fund.code} · {fund.fiscalYear}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label htmlFor="serial-status" className={formLabelClass()}>
            {t('common.status')}
          </label>
          <select
            id="serial-status"
            className={formControlClass()}
            value={values.status}
            onChange={(event) => onChange({ status: event.target.value as SubscriptionFormValues['status'] })}
          >
            <option value="active">{t('serials.status.active')}</option>
            <option value="expired">{t('serials.status.expired')}</option>
            <option value="closed">{t('serials.status.closed')}</option>
          </select>
        </div>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <Input
          id="serial-cost"
          label={t('serials.form.cost')}
          value={values.costEuros}
          onChange={(event) => onChange({ costEuros: event.target.value })}
          inputMode="decimal"
          error={fieldError('costEuros')}
          autoComplete="off"
        />
        <Input
          id="serial-issue-price"
          label={t('serials.form.issuePrice')}
          value={values.issuePriceEuros}
          onChange={(event) => onChange({ issuePriceEuros: event.target.value })}
          inputMode="decimal"
          error={fieldError('issuePriceEuros')}
          autoComplete="off"
        />
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <div>
          <label htmlFor="serial-frequency" className={formLabelClass()}>
            {t('serials.form.frequency')}
          </label>
          <select
            id="serial-frequency"
            className={formControlClass({ error: Boolean(errors.frequencyId) })}
            value={values.frequencyId}
            disabled={predictionLocked}
            aria-invalid={errors.frequencyId ? true : undefined}
            aria-describedby={
              [previewError ? previewErrorId : null, errors.frequencyId ? 'serial-frequency-error' : null]
                .filter(Boolean)
                .join(' ') || undefined
            }
            onChange={(event) => onChange({ frequencyId: event.target.value })}
          >
            <option value="">{t('serials.form.frequency')}</option>
            {frequencies.map((row) => (
              <option key={row.id} value={row.id}>
                {frequencyLabel(t, row)}
              </option>
            ))}
          </select>
          {errors.frequencyId ? (
            <p id="serial-frequency-error" className="mt-1 text-sm text-red-600 dark:text-red-400">
              {fieldError('frequencyId')}
            </p>
          ) : null}
        </div>
        <label className={`${formChoiceLabelClass()} sm:mt-7`}>
          <input
            id="serial-create-items"
            type="checkbox"
            className={formChoiceControlClass()}
            checked={values.createItems}
            onChange={(event) => onChange({ createItems: event.target.checked })}
          />
          {t('serials.form.createItems')}
        </label>
      </div>

      <div className="grid gap-3 sm:grid-cols-2">
        <Input
          id="serial-first-date"
          label={t('serials.form.firstIssueDate')}
          type="date"
          value={values.firstIssueDate}
          onChange={(event) => onChange({ firstIssueDate: event.target.value })}
          error={fieldError('firstIssueDate')}
          disabled={predictionLocked}
        />
        <Input
          id="serial-first-number"
          label={t('serials.form.firstNumber')}
          value={values.x}
          onChange={(event) => onChange({ x: event.target.value })}
          inputMode="numeric"
          error={fieldError('x')}
          disabled={predictionLocked}
        />
        <div>
          <label htmlFor="serial-length-kind" className={formLabelClass()}>
            {t('serials.form.lengthKind')}
          </label>
          <select
            id="serial-length-kind"
            className={formControlClass()}
            value={values.lengthKind}
            disabled={predictionLocked}
            onChange={(event) => onChange({ lengthKind: event.target.value as SubscriptionFormValues['lengthKind'] })}
          >
            <option value="months">{t('serials.form.length.months')}</option>
            <option value="weeks">{t('serials.form.length.weeks')}</option>
            <option value="issues">{t('serials.form.length.issues')}</option>
          </select>
        </div>
        <Input
          id="serial-length-count"
          label={t('serials.form.lengthCount')}
          value={values.lengthCount}
          onChange={(event) => onChange({ lengthCount: event.target.value })}
          inputMode="numeric"
          error={fieldError('lengthCount')}
          disabled={predictionLocked}
        />
      </div>

      <Input
        id="serial-grace"
        label={t('serials.form.graceDays')}
        value={values.graceDays}
        onChange={(event) => onChange({ graceDays: event.target.value })}
        inputMode="numeric"
        placeholder={gracePlaceholder == null ? undefined : String(gracePlaceholder)}
        hint={
          gracePlaceholder == null
            ? t('serials.form.graceNone')
            : t('serials.form.graceHint', { days: gracePlaceholder })
        }
        error={fieldError('graceDays')}
      />

      <CollapsibleFormSection
        title={t('serials.form.advanced')}
        summary={pattern ? `${pattern.label} · ${pattern.formula}` : null}
        open={advancedOpen}
        onToggle={() => setAdvancedOpen((open) => !open)}
      >
        <div>
          <label htmlFor="serial-pattern" className={formLabelClass()}>
            {t('serials.form.pattern')}
          </label>
          <select
            id="serial-pattern"
            className={formControlClass({ error: Boolean(errors.numberingPatternId) })}
            value={values.numberingPatternId}
            disabled={predictionLocked}
            aria-invalid={errors.numberingPatternId ? true : undefined}
            aria-describedby={errors.numberingPatternId ? 'serial-pattern-error' : undefined}
            onChange={(event) => onChange({ numberingPatternId: event.target.value })}
          >
            <option value="">{t('serials.form.pattern')}</option>
            {patterns.map((row) => (
              <option key={row.id} value={row.id}>
                {row.label}
              </option>
            ))}
          </select>
          {pattern ? <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{pattern.formula}</p> : null}
          {errors.numberingPatternId ? (
            <p id="serial-pattern-error" className="mt-1 text-sm text-red-600 dark:text-red-400">
              {fieldError('numberingPatternId')}
            </p>
          ) : null}
        </div>
        <Input
          id="serial-skips"
          label={t('serials.form.recurringSkips')}
          value={values.recurringSkips}
          onChange={(event) => onChange({ recurringSkips: event.target.value })}
          hint={t('serials.form.recurringSkipsHint')}
          error={fieldError('recurringSkips')}
          disabled={predictionLocked}
        />
        <Input
          id="serial-irregularities"
          label={t('serials.form.irregularities')}
          value={values.irregularities}
          onChange={(event) => onChange({ irregularities: event.target.value })}
          hint={t('serials.form.irregularitiesHint')}
          error={fieldError('irregularities')}
          disabled={predictionLocked}
        />
        <div className="grid gap-3 sm:grid-cols-2">
          <Input
            id="serial-call-number"
            label={t('serials.form.callNumber')}
            value={values.callNumber}
            onChange={(event) => onChange({ callNumber: event.target.value })}
          />
          <Input
            id="serial-location"
            label={t('serials.form.location')}
            value={values.location}
            onChange={(event) => onChange({ location: event.target.value })}
          />
        </div>
        <Input
          id="serial-holdings"
          label={t('serials.form.holdings')}
          value={values.holdingsStatement}
          onChange={(event) => onChange({ holdingsStatement: event.target.value })}
        />
        <Input
          id="serial-notes"
          label={t('serials.form.notes')}
          value={values.notes}
          onChange={(event) => onChange({ notes: event.target.value })}
        />
        <label className={formChoiceLabelClass()}>
          <input
            id="serial-library-only"
            type="checkbox"
            className={formChoiceControlClass()}
            checked={values.latestIssueLibraryUseOnly}
            onChange={(event) => onChange({ latestIssueLibraryUseOnly: event.target.checked })}
          />
          {t('serials.form.latestLibraryOnly')}
        </label>
      </CollapsibleFormSection>

      {predictionLocked ? (
        <p className="text-sm text-gray-600 dark:text-gray-300">{t('serials.form.predictionLocked')}</p>
      ) : null}

      <div aria-live="polite" aria-atomic="true" className="rounded-lg border border-gray-200 dark:border-gray-700">
        <table className="w-full text-sm">
          <caption className="px-3 py-2 text-left text-sm font-medium text-gray-900 dark:text-white">
            {t('serials.form.previewCaption')}
          </caption>
          <thead className="text-left text-gray-600 dark:text-gray-300">
            <tr>
              <th scope="col" className="px-3 py-1 font-medium">{t('serials.issues.label')}</th>
              <th scope="col" className="px-3 py-1 font-medium">{t('serials.issues.plannedDate')}</th>
            </tr>
          </thead>
          <tbody>
            {previewLoading ? (
              <tr>
                <td colSpan={2} className="px-3 py-2 text-gray-500 dark:text-gray-400">{t('serials.form.previewLoading')}</td>
              </tr>
            ) : preview.length === 0 ? (
              <tr>
                <td colSpan={2} className="px-3 py-2 text-gray-500 dark:text-gray-400">{t('serials.form.previewEmpty')}</td>
              </tr>
            ) : (
              preview.map((issue) => (
                <tr key={`${issue.slot}-${issue.sequence}`} className="border-t border-gray-100 dark:border-gray-800">
                  <td className="px-3 py-1.5 text-gray-900 dark:text-gray-100">{issue.displayLabel}</td>
                  <td className="px-3 py-1.5 text-gray-700 dark:text-gray-300">
                    {formatIsoDate(issue.plannedDate, i18n.language)}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
        {previewError ? (
          <p id={previewErrorId} role="alert" className="px-3 pb-2 text-sm text-red-600 dark:text-red-400">
            {emptyHorizon ? t('serials.form.emptyHorizon') : previewError}
          </p>
        ) : null}
      </div>
    </div>
  );
}
