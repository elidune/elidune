import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import Input from '@/components/common/Input';
import Modal from '@/components/common/Modal';

interface ReceiveIssueDialogProps {
  isOpen: boolean;
  label: string;
  createItems: boolean;
  isSaving: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (barcode: string) => void;
}

export default function ReceiveIssueDialog({
  isOpen,
  label,
  createItems,
  isSaving,
  error,
  onClose,
  onSubmit,
}: ReceiveIssueDialogProps) {
  const { t } = useTranslation();
  const [barcode, setBarcode] = useState('');

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={t('serials.receive.title')}
      size="md"
      footer={
        <div className="flex flex-wrap justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" form="serial-receive-form" isLoading={isSaving}>
            {t('serials.receive.confirm')}
          </Button>
        </div>
      }
    >
      <form
        id="serial-receive-form"
        className="space-y-3"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit(barcode.trim());
        }}
      >
        <p className="text-sm text-gray-800 dark:text-gray-100">{label}</p>
        {createItems ? (
          <Input
            id="serial-receive-barcode"
            label={t('serials.receive.barcode')}
            value={barcode}
            onChange={(event) => setBarcode(event.target.value)}
            hint={t('serials.receive.barcodeHint')}
            autoFocus
            autoComplete="off"
          />
        ) : (
          <p className="text-sm text-gray-600 dark:text-gray-300">{t('serials.receive.noCopy')}</p>
        )}
        {error ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {error}
          </p>
        ) : null}
      </form>
    </Modal>
  );
}

export function ReceivedSerialCopy({
  label,
  barcode,
  onDismiss,
}: {
  label: string;
  barcode: string | null;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div
      className="rounded-lg border border-gray-200 bg-white p-3 dark:border-gray-700 dark:bg-gray-900"
      aria-live="polite"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h2 className="text-base font-semibold text-gray-900 dark:text-white">{t('serials.receive.resultTitle')}</h2>
          <p className="mt-1 text-sm text-gray-800 dark:text-gray-100">{label}</p>
          <p className="mt-1 text-sm font-medium text-gray-900 dark:text-white">
            {barcode?.trim() || t('serials.receive.noBarcode')}
          </p>
        </div>
        <button
          type="button"
          className="text-sm font-medium text-amber-800 underline-offset-2 hover:underline dark:text-amber-200"
          onClick={onDismiss}
        >
          {t('serials.receive.dismiss')}
        </button>
      </div>
    </div>
  );
}
