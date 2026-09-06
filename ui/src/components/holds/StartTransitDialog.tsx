import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Button, Input, Modal } from '@/components/common';
import api from '@/services/api';
import type { CreateTransit, Hold, Item } from '@/types';
import { formChoiceLabelClass, formControlClass, formLabelClass } from '@/utils/formControl';
import { copyAlreadyAtPickupSite } from '@/utils/transitDisplay';

export default function StartTransitDialog({
  hold,
  isOpen,
  isLoading,
  error,
  onClose,
  onConfirm,
}: {
  hold: Hold | null;
  isOpen: boolean;
  isLoading: boolean;
  error?: string | null;
  onClose: () => void;
  onConfirm: (data: CreateTransit) => void;
}) {
  const { t } = useTranslation();
  const [itemId, setItemId] = useState(hold?.itemId ?? '');
  const [notes, setNotes] = useState('');
  const [ship, setShip] = useState(false);

  useEffect(() => {
    if (isOpen) {
      setItemId(hold?.itemId ?? '');
      setNotes('');
      setShip(false);
    }
  }, [hold?.itemId, isOpen]);

  const biblioId = hold?.biblioId;
  const copiesQuery = useQuery({
    queryKey: ['biblio', biblioId],
    queryFn: () => api.getBiblio(biblioId!),
    enabled: isOpen && !!biblioId && !hold?.itemId,
    staleTime: 60 * 1000,
  });

  const copies: Item[] = copiesQuery.data?.items ?? [];
  const pinned = hold?.itemId ?? '';
  const selectedCopy = pinned || itemId;
  const selectedSourceId = copies.find((c) => c.id === selectedCopy)?.sourceId;
  const sameSite = hold ? copyAlreadyAtPickupSite(hold, selectedSourceId) : false;
  const needsCopy = !pinned;
  const canSubmit = !!hold && !!selectedCopy && !sameSite && !isLoading;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('holds.startTransitTitle')}
      size="md"
      footer={
        <div className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isLoading}>
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            variant="primary"
            isLoading={isLoading}
            disabled={!canSubmit}
            onClick={() =>
              onConfirm({
                itemId: selectedCopy || undefined,
                notes: notes.trim() || undefined,
                ship,
              })
            }
          >
            {t('holds.startTransit')}
          </Button>
        </div>
      }
    >
      <div className="space-y-3 text-sm text-gray-700 dark:text-gray-300">
        <p>{t('holds.startTransitHint')}</p>
        {needsCopy && (
          <div className="flex flex-col gap-1">
            <label className={formLabelClass({ marginBottom: false })} htmlFor="start-transit-copy">
              {t('holds.startTransitPickCopy')}
            </label>
            {copiesQuery.isLoading ? (
              <p className="text-xs text-gray-500">{t('common.loading')}</p>
            ) : (
              <select
                id="start-transit-copy"
                value={itemId}
                onChange={(e) => setItemId(e.target.value)}
                className={formControlClass()}
              >
                <option value="">{t('holds.selectCopy')}</option>
                {copies.map((it) => (
                  <option key={it.id} value={it.id} disabled={it.borrowed === true}>
                    {it.barcode || it.callNumber || it.id}
                    {it.sourceName ? ` — ${it.sourceName}` : ''}
                    {it.borrowed ? ` (${t('holds.hintBorrowed')})` : ''}
                  </option>
                ))}
              </select>
            )}
          </div>
        )}
        {sameSite && (
          <p className="text-sm text-amber-700 dark:text-amber-300">{t('holds.startTransitSameSite')}</p>
        )}
        <label className={formChoiceLabelClass()}>
          <input
            type="checkbox"
            className="text-indigo-600"
            checked={ship}
            onChange={(e) => setShip(e.target.checked)}
          />
          {t('holds.startTransitShipNow')}
        </label>
        <Input
          label={t('holds.startTransitNotes')}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />
        {error ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        ) : null}
      </div>
    </Modal>
  );
}
