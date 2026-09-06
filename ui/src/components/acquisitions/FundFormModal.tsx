import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Modal } from '@/components/common';
import { moneyInputToApi } from '@/utils/acquisitionDisplay';
import { formLabelClass, formTextareaClass } from '@/utils/formControl';
import type { AcquisitionFund, CreateFund, UpdateFund } from '@/types';

interface FundFormModalProps {
  fund: AcquisitionFund | null;
  isOpen: boolean;
  isSaving: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (data: CreateFund | UpdateFund) => void;
}

export default function FundFormModal({
  fund,
  isOpen,
  isSaving,
  error,
  onClose,
  onSubmit,
}: FundFormModalProps) {
  const { t } = useTranslation();
  const [code, setCode] = useState(fund?.code ?? '');
  const [name, setName] = useState(fund?.name ?? '');
  const [fiscalYear, setFiscalYear] = useState(String(fund?.fiscalYear ?? new Date().getFullYear()));
  const [allocated, setAllocated] = useState(
    fund?.allocatedAmount != null ? String(fund.allocatedAmount) : '',
  );
  const [currency, setCurrency] = useState(fund?.currency ?? 'EUR');
  const [notes, setNotes] = useState(fund?.notes ?? '');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const year = Number(fiscalYear);
    if (!code.trim() || !name.trim() || !Number.isInteger(year) || isSaving) return;
    onSubmit({
      code: code.trim(),
      name: name.trim(),
      fiscalYear: year,
      allocatedAmount: moneyInputToApi(allocated),
      currency: currency.trim().toUpperCase() || 'EUR',
      notes: notes.trim() || null,
    });
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={fund ? t('acquisitions.funds.editTitle') : t('acquisitions.funds.createTitle')}
      size="md"
      footer={
        <div className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel')}
          </Button>
          <Button
            type="submit"
            form="fund-form"
            isLoading={isSaving}
            disabled={!code.trim() || !name.trim()}
          >
            {t('common.save')}
          </Button>
        </div>
      }
    >
      <form id="fund-form" className="space-y-3" onSubmit={handleSubmit}>
        <Input
          label={t('acquisitions.funds.code')}
          value={code}
          onChange={(e) => setCode(e.target.value)}
          required
          autoFocus
        />
        <Input
          label={t('acquisitions.funds.name')}
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
        />
        <Input
          label={t('acquisitions.funds.fiscalYear')}
          type="number"
          value={fiscalYear}
          onChange={(e) => setFiscalYear(e.target.value)}
          required
        />
        <Input
          label={t('acquisitions.funds.allocated')}
          value={allocated}
          onChange={(e) => setAllocated(e.target.value)}
          inputMode="decimal"
        />
        <Input
          label={t('acquisitions.funds.currency')}
          value={currency}
          onChange={(e) => setCurrency(e.target.value.toUpperCase())}
          maxLength={3}
        />
        <div>
          <label htmlFor="fund-notes" className={formLabelClass()}>
            {t('acquisitions.funds.notes')}
          </label>
          <textarea
            id="fund-notes"
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
