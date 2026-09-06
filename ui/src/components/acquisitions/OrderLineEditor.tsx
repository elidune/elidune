import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input } from '@/components/common';
import api from '@/services/api';
import { moneyInputToApi } from '@/utils/acquisitionDisplay';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import { getApiErrorMessage } from '@/utils/apiError';
import type { AcquisitionFund, BiblioShort, CreateOrderLine, PurchaseOrderLine } from '@/types';

type LineIntent = 'biblio' | 'isbn' | 'title';

interface OrderLineEditorProps {
  funds: AcquisitionFund[];
  defaultFundId?: string | null;
  initial?: PurchaseOrderLine | null;
  isSaving: boolean;
  error: string | null;
  onCancel: () => void;
  onSubmit: (data: CreateOrderLine) => void;
}

export default function OrderLineEditor({
  funds,
  defaultFundId,
  initial,
  isSaving,
  error,
  onCancel,
  onSubmit,
}: OrderLineEditorProps) {
  const { t } = useTranslation();
  const [intent, setIntent] = useState<LineIntent>(() => {
    if (initial?.biblioId) return 'biblio';
    if (initial?.isbn && !initial.title) return 'isbn';
    return 'title';
  });
  const [biblioDraft, setBiblioDraft] = useState('');
  const [biblioResults, setBiblioResults] = useState<BiblioShort[]>([]);
  const [biblioSearching, setBiblioSearching] = useState(false);
  const [biblioSearchError, setBiblioSearchError] = useState<string | null>(null);
  const biblioSeqRef = useRef(0);
  const [selectedBiblio, setSelectedBiblio] = useState<BiblioShort | null>(
    initial?.biblioId
      ? { id: initial.biblioId, title: initial.title ?? undefined, isbn: initial.isbn }
      : null,
  );
  const [isbn, setIsbn] = useState(initial?.isbn ?? '');
  const [title, setTitle] = useState(initial?.title ?? '');
  const [quantity, setQuantity] = useState(String(initial?.quantityOrdered ?? 1));
  const [unitPrice, setUnitPrice] = useState(
    initial?.unitPrice != null ? String(initial.unitPrice) : '',
  );
  const [currency, setCurrency] = useState(initial?.currency ?? 'EUR');
  const [fundId, setFundId] = useState(initial?.fundId ?? defaultFundId ?? '');
  const [notes, setNotes] = useState(initial?.notes ?? '');

  useEffect(() => {
    const q = biblioDraft.trim();
    if (!q || intent !== 'biblio') {
      biblioSeqRef.current += 1;
      return;
    }
    const timer = window.setTimeout(() => {
      const seq = ++biblioSeqRef.current;
      setBiblioSearching(true);
      void (async () => {
        try {
          const res = await api.getBiblios({ freesearch: q, perPage: 12, page: 1 });
          if (seq !== biblioSeqRef.current) return;
          setBiblioSearchError(null);
          setBiblioResults(res.items);
        } catch (err) {
          if (seq !== biblioSeqRef.current) return;
          setBiblioResults([]);
          setBiblioSearchError(getApiErrorMessage(err, t) || t('acquisitions.lines.biblioLoadError'));
        } finally {
          if (seq === biblioSeqRef.current) setBiblioSearching(false);
        }
      })();
    }, 350);
    return () => window.clearTimeout(timer);
  }, [biblioDraft, intent, t]);

  const qty = Number(quantity);
  const canSave =
    Number.isInteger(qty) &&
    qty >= 1 &&
    !isSaving &&
    (intent === 'biblio'
      ? Boolean(selectedBiblio?.id)
      : intent === 'isbn'
        ? Boolean(isbn.trim())
        : Boolean(title.trim()));

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSave) return;
    onSubmit({
      fundId: fundId || null,
      biblioId: intent === 'biblio' ? selectedBiblio?.id ?? null : null,
      isbn: intent === 'isbn' ? isbn.trim() : intent === 'biblio' ? selectedBiblio?.isbn ?? null : null,
      title:
        intent === 'title'
          ? title.trim()
          : intent === 'biblio'
            ? selectedBiblio?.title ?? null
            : null,
      quantity: qty,
      unitPrice: moneyInputToApi(unitPrice),
      currency: currency.trim().toUpperCase() || 'EUR',
      notes: notes.trim() || null,
    });
  };

  return (
    <form className="space-y-3" onSubmit={handleSubmit}>
      <fieldset className="space-y-2">
        <legend className={formLabelClass()}>{t('acquisitions.lines.intent')}</legend>
        <div className="flex flex-wrap gap-2">
          {(['biblio', 'isbn', 'title'] as const).map((value) => (
            <label
              key={value}
              className={`cursor-pointer rounded-lg border px-3 py-1.5 text-sm ${
                intent === value
                  ? 'border-amber-400 bg-amber-50 text-amber-800 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-200'
                  : 'border-gray-200 text-gray-700 dark:border-gray-700 dark:text-gray-300'
              }`}
            >
              <input
                type="radio"
                className="sr-only"
                name="line-intent"
                checked={intent === value}
                onChange={() => setIntent(value)}
              />
              {t(`acquisitions.lines.intent${value[0].toUpperCase()}${value.slice(1)}`)}
            </label>
          ))}
        </div>
        <p className="text-xs text-gray-500 dark:text-gray-400">{t('acquisitions.lines.intentHelp')}</p>
      </fieldset>

      {intent === 'biblio' ? (
        <div>
          <label htmlFor="line-biblio" className={formLabelClass()}>
            {t('acquisitions.lines.biblio')}
          </label>
          {selectedBiblio ? (
            <div className="flex items-center justify-between gap-2 rounded-lg border border-gray-200 px-3 py-2 text-sm dark:border-gray-700">
              <span className="min-w-0 truncate text-gray-900 dark:text-white">
                {selectedBiblio.title || selectedBiblio.isbn || selectedBiblio.id}
              </span>
              <Button
                type="button"
                size="sm"
                variant="secondary"
                onClick={() => {
                  setSelectedBiblio(null);
                  setBiblioDraft('');
                }}
              >
                {t('common.clear')}
              </Button>
            </div>
          ) : (
            <>
              <input
                id="line-biblio"
                className={formControlClass()}
                value={biblioDraft}
                onChange={(e) => setBiblioDraft(e.target.value)}
                placeholder={t('acquisitions.lines.biblioPlaceholder')}
              />
              {biblioSearching ? (
                <p className="mt-1 text-xs text-gray-500">{t('common.loading')}</p>
              ) : null}
              {biblioSearchError ? (
                <p role="alert" className="mt-1 text-xs text-red-600 dark:text-red-400">
                  {biblioSearchError}
                </p>
              ) : null}
              {biblioResults.length > 0 ? (
                <ul className="mt-1 max-h-40 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700">
                  {biblioResults.map((b) => (
                    <li key={b.id}>
                      <button
                        type="button"
                        className="w-full px-3 py-2 text-left text-sm hover:bg-gray-50 dark:hover:bg-gray-800"
                        onClick={() => {
                          setSelectedBiblio(b);
                          setBiblioDraft('');
                          setBiblioResults([]);
                        }}
                      >
                        <span className="block text-gray-900 dark:text-white">
                          {b.title || t('acquisitions.lines.untitled')}
                        </span>
                        <span className="block text-xs text-gray-500">
                          {[b.isbn, b.id].filter(Boolean).join(' · ')}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              ) : null}
            </>
          )}
        </div>
      ) : null}

      {intent === 'isbn' ? (
        <Input
          label={t('acquisitions.lines.isbn')}
          value={isbn}
          onChange={(e) => setIsbn(e.target.value)}
        />
      ) : null}

      {intent === 'title' ? (
        <Input
          label={t('acquisitions.lines.title')}
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
      ) : null}

      <div className="grid gap-3 sm:grid-cols-3">
        <Input
          label={t('acquisitions.lines.quantity')}
          type="number"
          min={1}
          value={quantity}
          onChange={(e) => setQuantity(e.target.value)}
          required
        />
        <Input
          label={t('acquisitions.lines.unitPrice')}
          value={unitPrice}
          onChange={(e) => setUnitPrice(e.target.value)}
          inputMode="decimal"
        />
        <Input
          label={t('acquisitions.funds.currency')}
          value={currency}
          onChange={(e) => setCurrency(e.target.value.toUpperCase())}
          maxLength={3}
        />
      </div>

      <div>
        <label htmlFor="line-fund" className={formLabelClass()}>
          {t('acquisitions.funds.fund')}
        </label>
        <select
          id="line-fund"
          className={formControlClass()}
          value={fundId}
          onChange={(e) => setFundId(e.target.value)}
        >
          <option value="">{t('acquisitions.funds.none')}</option>
          {funds.map((f) => (
            <option key={f.id} value={f.id}>
              {f.code} · {f.fiscalYear} · {f.name}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label htmlFor="line-notes" className={formLabelClass()}>
          {t('acquisitions.lines.notes')}
        </label>
        <textarea
          id="line-notes"
          className={formTextareaClass()}
          rows={2}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />
      </div>

      {error ? (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          {error}
        </p>
      ) : null}

      <div className="flex justify-end gap-2">
        <Button type="button" variant="secondary" onClick={onCancel} disabled={isSaving}>
          {t('common.cancel')}
        </Button>
        <Button type="submit" isLoading={isSaving} disabled={!canSave}>
          {t('common.save')}
        </Button>
      </div>
    </form>
  );
}
