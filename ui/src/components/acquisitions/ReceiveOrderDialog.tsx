import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Modal } from '@/components/common';
import { useSourcesQuery } from '@/hooks/holds/useSourcesQuery';
import type { PurchaseOrderLine, ReceiveItemSpec, ReceivePurchaseOrder } from '@/types';
import { formatMoney, lineIntentLabel, quantityRemaining } from '@/utils/acquisitionDisplay';
import { formControlClass, formLabelClass, formTextareaClass } from '@/utils/formControl';

interface ReceiveDraftLine {
  lineId: string;
  quantity: string;
  unitPrice: string;
  items: { barcode: string; sourceId: string; price: string; callNumber: string }[];
}

function emptyItem(price = ''): ReceiveDraftLine['items'][number] {
  return { barcode: '', sourceId: '', price, callNumber: '' };
}

function syncItems(
  items: ReceiveDraftLine['items'],
  qty: number,
  defaultPrice: string,
): ReceiveDraftLine['items'] {
  const next = items.slice(0, Math.max(0, qty));
  while (next.length < qty) next.push(emptyItem(defaultPrice));
  return next;
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

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isSaving) return;
    const payloadLines = drafts
      .map((d) => {
        const quantity = Number(d.quantity);
        if (!Number.isInteger(quantity) || quantity < 1) return null;
        const items: ReceiveItemSpec[] = d.items.slice(0, quantity).map((item) => ({
          barcode: item.barcode.trim() || null,
          sourceId: item.sourceId || null,
          price: item.price.trim() || d.unitPrice.trim() || null,
          callNumber: item.callNumber.trim() || null,
        }));
        return {
          lineId: d.lineId,
          quantity,
          unitPrice: d.unitPrice.trim() || null,
          items,
        };
      })
      .filter((line): line is NonNullable<typeof line> => line != null);

    if (payloadLines.length === 0) return;
    onSubmit({
      notes: notes.trim() || null,
      lines: payloadLines,
    });
  };

  const canSubmit = drafts.some((d) => {
    const qty = Number(d.quantity);
    return Number.isInteger(qty) && qty >= 1;
  });

  return (
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
        {receivable.length === 0 ? (
          <p className="text-sm text-gray-500">{t('acquisitions.receive.nothingLeft')}</p>
        ) : (
          receivable.map((line) => {
            const draft = drafts.find((d) => d.lineId === line.id);
            if (!draft) return null;
            const remaining = quantityRemaining(line);
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
                {draft.items.map((item, idx) => (
                  <div
                    key={`${line.id}-${idx}`}
                    className="grid gap-2 rounded-lg bg-gray-50 p-2 dark:bg-gray-900/60 sm:grid-cols-2"
                  >
                    <p className="sm:col-span-2 text-xs font-medium text-gray-500">
                      {t('acquisitions.receive.copyN', { n: idx + 1 })}
                    </p>
                    <Input
                      label={t('acquisitions.receive.barcode')}
                      value={item.barcode}
                      onChange={(e) => {
                        const items = draft.items.map((it, i) =>
                          i === idx ? { ...it, barcode: e.target.value } : it,
                        );
                        updateDraft(line.id, { items });
                      }}
                    />
                    <div>
                      <label className={formLabelClass()} htmlFor={`src-${line.id}-${idx}`}>
                        {t('acquisitions.receive.source')}
                      </label>
                      <select
                        id={`src-${line.id}-${idx}`}
                        className={formControlClass()}
                        value={item.sourceId}
                        onChange={(e) => {
                          const items = draft.items.map((it, i) =>
                            i === idx ? { ...it, sourceId: e.target.value } : it,
                          );
                          updateDraft(line.id, { items });
                        }}
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
                      onChange={(e) => {
                        const items = draft.items.map((it, i) =>
                          i === idx ? { ...it, price: e.target.value } : it,
                        );
                        updateDraft(line.id, { items });
                      }}
                    />
                    <Input
                      label={t('acquisitions.receive.callNumber')}
                      value={item.callNumber}
                      onChange={(e) => {
                        const items = draft.items.map((it, i) =>
                          i === idx ? { ...it, callNumber: e.target.value } : it,
                        );
                        updateDraft(line.id, { items });
                      }}
                    />
                  </div>
                ))}
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
  );
}
