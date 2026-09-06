import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge, Button, ConfirmDialog, Input, Modal } from '@/components/common';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import type { PurchaseOrderLine, ReceiveItemSpec, ReceivePurchaseOrder } from '@/types';
import { formatMoney, lineIntentLabel, moneyInputToApi, quantityRemaining } from '@/utils/acquisitionDisplay';
import { getApiErrorMessage } from '@/utils/apiError';
import { isReadyToCirculate } from '@/utils/circulationReadiness';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';

interface ReceiveDraftItem {
  barcode: string;
  sourceId: string;
  price: string;
  priceDeferred: boolean;
  callNumber: string;
}

interface ReceiveDraftLine {
  lineId: string;
  quantity: string;
  unitPrice: string;
  items: ReceiveDraftItem[];
}

function emptyItem(price = ''): ReceiveDraftItem {
  return { barcode: '', sourceId: '', price, priceDeferred: false, callNumber: '' };
}

function syncItems(items: ReceiveDraftItem[], qty: number, defaultPrice: string): ReceiveDraftItem[] {
  const next = items.slice(0, Math.max(0, qty));
  while (next.length < qty) next.push(emptyItem(defaultPrice));
  return next;
}

function draftItemToSpec(item: ReceiveDraftItem, unitPrice: string | null): ReceiveItemSpec {
  return {
    barcode: item.barcode.trim() || null,
    sourceId: item.sourceId || null,
    price: item.priceDeferred ? null : moneyInputToApi(item.price) || unitPrice,
    priceDeferred: item.priceDeferred || undefined,
    callNumber: item.callNumber.trim() || null,
  };
}

function specIsReady(spec: ReceiveItemSpec): boolean {
  return isReadyToCirculate({
    barcode: spec.barcode,
    sourceId: spec.sourceId,
    sourceName: spec.sourceName,
    price: spec.price,
    priceDeferred: spec.priceDeferred,
  });
}

interface ReceiveOrderDialogProps {
  isOpen: boolean;
  lines: PurchaseOrderLine[];
  isSaving: boolean;
  error: string | null;
  locale: string;
  onClose: () => void;
  onSubmit: (data: ReceivePurchaseOrder) => void;
}

export default function ReceiveOrderDialog({
  isOpen,
  lines,
  isSaving,
  error,
  locale,
  onClose,
  onSubmit,
}: ReceiveOrderDialogProps) {
  const { t } = useTranslation();
  const sourcesQuery = useSourcesQuery(isOpen);
  const receivable = useMemo(
    () => lines.filter((line) => quantityRemaining(line) > 0),
    [lines],
  );
  const [notes, setNotes] = useState('');
  const [pending, setPending] = useState<ReceivePurchaseOrder | null>(null);
  const [drafts, setDrafts] = useState<ReceiveDraftLine[]>(() =>
    receivable.map((line) => {
      const remaining = quantityRemaining(line);
      const price = line.unitPrice != null ? String(line.unitPrice) : '';
      return {
        lineId: line.id,
        quantity: String(remaining),
        unitPrice: price,
        items: syncItems([], remaining, price),
      };
    }),
  );

  const updateDraft = (lineId: string, patch: Partial<ReceiveDraftLine>) => {
    setDrafts((prev) =>
      prev.map((d) => {
        if (d.lineId !== lineId) return d;
        const next = { ...d, ...patch };
        const qty = Number(next.quantity);
        if (Number.isInteger(qty) && qty >= 0) {
          next.items = syncItems(next.items, qty, next.unitPrice);
        }
        return next;
      }),
    );
  };

  const patchItem = (lineId: string, idx: number, patch: Partial<ReceiveDraftItem>) => {
    const draft = drafts.find((d) => d.lineId === lineId);
    if (!draft) return;
    const items = draft.items.map((it, i) => (i === idx ? { ...it, ...patch } : it));
    updateDraft(lineId, { items });
  };

  const buildPayload = (): ReceivePurchaseOrder | null => {
    const payloadLines = drafts
      .map((d) => {
        const line = lines.find((l) => l.id === d.lineId);
        const remaining = line ? quantityRemaining(line) : 0;
        const quantity = Math.min(Number(d.quantity), remaining);
        if (!Number.isInteger(quantity) || quantity < 1) return null;
        const unitPrice = moneyInputToApi(d.unitPrice);
        const items: ReceiveItemSpec[] = d.items.slice(0, quantity).map((item) =>
          draftItemToSpec(item, unitPrice),
        );
        return { lineId: d.lineId, quantity, unitPrice, items };
      })
      .filter((line): line is NonNullable<typeof line> => line != null);

    if (payloadLines.length === 0) return null;
    return { notes: notes.trim() || null, lines: payloadLines };
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isSaving) return;
    const payload = buildPayload();
    if (!payload) return;
    const incomplete = payload.lines.reduce(
      (n, line) => n + (line.items ?? []).filter((item) => !specIsReady(item)).length,
      0,
    );
    if (incomplete > 0) {
      setPending(payload);
      return;
    }
    onSubmit(payload);
  };

  const canSubmit = drafts.some((d) => {
    const qty = Number(d.quantity);
    return Number.isInteger(qty) && qty >= 1;
  });

  return (
    <>
      <Modal
        isOpen={isOpen}
        onClose={onClose}
        title={t('acquisitions.receive.title')}
        size="lg"
        footer={
          <div className="flex justify-end gap-2">
            <Button type="button" variant="secondary" onClick={onClose} disabled={isSaving}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" form="receive-form" isLoading={isSaving} disabled={!canSubmit}>
              {t('acquisitions.receive.confirm')}
            </Button>
          </div>
        }
      >
        <form id="receive-form" className="space-y-5" onSubmit={handleSubmit}>
          <p className="text-sm text-gray-600 dark:text-gray-400">{t('acquisitions.receive.help')}</p>
          <p className="text-sm text-amber-800 dark:text-amber-300">{t('acquisitions.receive.incompleteHint')}</p>
          {sourcesQuery.isError ? (
            <p role="alert" className="text-sm text-red-600 dark:text-red-400">
              {getApiErrorMessage(sourcesQuery.error, t) || t('acquisitions.receive.sourcesLoadError')}
            </p>
          ) : null}
          {receivable.length === 0 ? (
            <p className="text-sm text-gray-500">{t('acquisitions.receive.nothingLeft')}</p>
          ) : (
            receivable.map((line) => {
              const draft = drafts.find((d) => d.lineId === line.id);
              if (!draft) return null;
              const remaining = quantityRemaining(line);
              const unitPrice = moneyInputToApi(draft.unitPrice);
              return (
                <section
                  key={line.id}
                  className="space-y-3 rounded-xl border border-gray-200 p-3 dark:border-gray-800"
                >
                  <div>
                    <h3 className="font-medium text-gray-900 dark:text-white">
                      {lineIntentLabel(line)}
                    </h3>
                    <p className="text-xs text-gray-500">
                      {t('acquisitions.receive.remaining', {
                        remaining,
                        ordered: line.quantityOrdered,
                        received: line.quantityReceived,
                      })}
                      {line.unitPrice != null
                        ? ` · ${formatMoney(line.unitPrice, line.currency, locale)}`
                        : ''}
                    </p>
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <Input
                      label={t('acquisitions.receive.quantity')}
                      type="number"
                      min={0}
                      max={remaining}
                      value={draft.quantity}
                      onChange={(e) => updateDraft(line.id, { quantity: e.target.value })}
                    />
                    <Input
                      label={t('acquisitions.lines.unitPrice')}
                      value={draft.unitPrice}
                      onChange={(e) => updateDraft(line.id, { unitPrice: e.target.value })}
                      inputMode="decimal"
                    />
                  </div>
                  {draft.items.map((item, idx) => {
                    const spec = draftItemToSpec(item, unitPrice);
                    const ready = specIsReady(spec);
                    return (
                      <div
                        key={`${line.id}-${idx}`}
                        className="grid gap-2 rounded-lg bg-gray-50 p-2 dark:bg-gray-900/60 sm:grid-cols-2"
                      >
                        <div className="sm:col-span-2 flex flex-wrap items-center justify-between gap-2">
                          <p className="text-xs font-medium text-gray-500">
                            {t('acquisitions.receive.copyN', { n: idx + 1 })}
                          </p>
                          {ready ? (
                            <Badge variant="success" size="sm">{t('acquisitions.receive.statusReady')}</Badge>
                          ) : (
                            <Badge variant="warning" size="sm">{t('acquisitions.receive.statusIncomplete')}</Badge>
                          )}
                        </div>
                        <Input
                          label={t('acquisitions.receive.barcode')}
                          value={item.barcode}
                          onChange={(e) => patchItem(line.id, idx, { barcode: e.target.value })}
                        />
                        <div>
                          <label className={formLabelClass()} htmlFor={`src-${line.id}-${idx}`}>
                            {t('acquisitions.receive.source')}
                          </label>
                          <select
                            id={`src-${line.id}-${idx}`}
                            className={formControlClass()}
                            value={item.sourceId}
                            onChange={(e) => patchItem(line.id, idx, { sourceId: e.target.value })}
                          >
                            <option value="">{t('acquisitions.receive.sourceNone')}</option>
                            {(sourcesQuery.data ?? []).map((s) => (
                              <option key={s.id} value={s.id}>
                                {s.name ?? s.key ?? s.id}
                              </option>
                            ))}
                          </select>
                        </div>
                        <Input
                          label={t('acquisitions.receive.price')}
                          value={item.price}
                          onChange={(e) => patchItem(line.id, idx, { price: e.target.value })}
                          disabled={item.priceDeferred}
                        />
                        <label className="flex items-start gap-2 self-end pb-1 text-sm text-gray-700 dark:text-gray-300">
                          <input
                            type="checkbox"
                            className="mt-1"
                            checked={item.priceDeferred}
                            onChange={(e) => patchItem(line.id, idx, { priceDeferred: e.target.checked })}
                          />
                          <span>{t('acquisitions.receive.priceDeferred')}</span>
                        </label>
                        <Input
                          label={t('acquisitions.receive.callNumber')}
                          value={item.callNumber}
                          onChange={(e) => patchItem(line.id, idx, { callNumber: e.target.value })}
                        />
                      </div>
                    );
                  })}
                </section>
              );
            })
          )}
          <div>
            <label htmlFor="receive-notes" className={formLabelClass()}>
              {t('acquisitions.receive.notes')}
            </label>
            <textarea
              id="receive-notes"
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
        </form>
      </Modal>
      <ConfirmDialog
        isOpen={pending != null}
        onClose={() => { if (!isSaving) setPending(null); }}
        onConfirm={() => {
          if (!pending || isSaving) return;
          const data = pending;
          setPending(null);
          onSubmit(data);
        }}
        title={t('acquisitions.receive.incompleteConfirmTitle')}
        message={t('acquisitions.receive.incompleteConfirm', {
          count: pending
            ? pending.lines.reduce(
                (n, line) => n + (line.items ?? []).filter((item) => !specIsReady(item)).length,
                0,
              )
            : 0,
        })}
        confirmLabel={t('acquisitions.receive.confirm')}
        isLoading={isSaving}
        stackOnTop
      />
    </>
  );
}
