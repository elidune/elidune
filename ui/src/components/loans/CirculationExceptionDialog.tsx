import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog, Input } from '@/components/common';
import { formChoiceLabelClass, formLabelClass, formTextareaClass } from '@/utils/formControl';
import type { CirculationExceptionKind, CirculationExceptionSubmit, CirculationExceptionTarget } from '@/hooks/loans/useCirculationExceptionAction';
import type { DamageDisposition } from '@/types';

interface CirculationExceptionDialogProps {
  target: CirculationExceptionTarget | null;
  isLoading: boolean;
  error: string | null;
  onClose: () => void;
  onConfirm: (payload: CirculationExceptionSubmit) => void;
}

function copyKey(kind: CirculationExceptionKind): { title: string; message: string; confirm: string } {
  switch (kind) {
    case 'lost':
      return {
        title: 'loans.exceptions.lostTitle',
        message: 'loans.exceptions.lostBilling',
        confirm: 'loans.exceptions.lost',
      };
    case 'damaged':
      return {
        title: 'loans.exceptions.damagedTitle',
        message: 'loans.exceptions.damagedBilling',
        confirm: 'loans.exceptions.damaged',
      };
    case 'claimedReturned':
      return {
        title: 'loans.exceptions.claimedTitle',
        message: 'loans.exceptions.claimedBilling',
        confirm: 'loans.exceptions.claimedReturned',
      };
    case 'resolveFound':
      return {
        title: 'loans.exceptions.resolveFoundTitle',
        message: 'loans.exceptions.resolveBilling',
        confirm: 'loans.exceptions.resolveFound',
      };
    case 'resolveNotFound':
      return {
        title: 'loans.exceptions.resolveNotFoundTitle',
        message: 'loans.exceptions.resolveBilling',
        confirm: 'loans.exceptions.resolveNotFound',
      };
  }
}

function isPositiveAmount(value: string): boolean {
  const n = Number(value.trim().replace(',', '.'));
  return Number.isFinite(n) && n > 0;
}

function ExceptionFields({
  target,
  isLoading,
  bill,
  setBill,
  amount,
  setAmount,
  disposition,
  setDisposition,
  notes,
  setNotes,
  inventoryChecked,
  setInventoryChecked,
  amountInvalid,
}: {
  target: CirculationExceptionTarget;
  isLoading: boolean;
  bill: boolean;
  setBill: (v: boolean) => void;
  amount: string;
  setAmount: (v: string) => void;
  disposition: DamageDisposition;
  setDisposition: (v: DamageDisposition) => void;
  notes: string;
  setNotes: (v: string) => void;
  inventoryChecked: boolean;
  setInventoryChecked: (v: boolean) => void;
  amountInvalid: boolean;
}) {
  const { t } = useTranslation();
  const kind = target.kind;
  const mayBill = kind === 'lost' || kind === 'damaged';
  const needsInventory = kind === 'resolveFound' || kind === 'resolveNotFound';

  return (
    <>
      {target.label ? (
        <p className="mt-3 text-sm font-medium text-gray-900 dark:text-white">{target.label}</p>
      ) : null}

      {kind === 'damaged' && (
        <fieldset className="mt-4 space-y-2">
          <legend className={formLabelClass()}>{t('loans.exceptions.disposition')}</legend>
          <label className={formChoiceLabelClass()}>
            <input
              type="radio"
              name="damage-disposition"
              value="return"
              checked={disposition === 'return'}
              onChange={() => setDisposition('return')}
              disabled={isLoading}
            />
            {t('loans.exceptions.dispositionReturn')}
          </label>
          <label className={formChoiceLabelClass()}>
            <input
              type="radio"
              name="damage-disposition"
              value="keep"
              checked={disposition === 'keep'}
              onChange={() => setDisposition('keep')}
              disabled={isLoading}
            />
            {t('loans.exceptions.dispositionKeep')}
          </label>
        </fieldset>
      )}

      {mayBill && (
        <div className="mt-4 space-y-3">
          <label className={formChoiceLabelClass()}>
            <input
              type="checkbox"
              checked={bill}
              onChange={(e) => setBill(e.target.checked)}
              disabled={isLoading}
            />
            {t(kind === 'lost' ? 'loans.exceptions.billReplacement' : 'loans.exceptions.billDamage')}
          </label>
          {bill && (
            <Input
              type="text"
              inputMode="decimal"
              label={t('loans.exceptions.amount')}
              hint={
                kind === 'lost'
                  ? t('loans.exceptions.amountHintLost')
                  : t('loans.exceptions.amountHintDamaged')
              }
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              disabled={isLoading}
              error={amountInvalid ? t('loans.exceptions.amountRequired') : undefined}
            />
          )}
        </div>
      )}

      {needsInventory && (
        <label className={`${formChoiceLabelClass()} mt-4`}>
          <input
            type="checkbox"
            checked={inventoryChecked}
            onChange={(e) => setInventoryChecked(e.target.checked)}
            disabled={isLoading}
          />
          {t('loans.exceptions.inventoryChecked')}
        </label>
      )}

      <div className="mt-4">
        <label className={formLabelClass()} htmlFor="circulation-exception-notes">
          {t('loans.exceptions.notes')}
        </label>
        <textarea
          id="circulation-exception-notes"
          className={formTextareaClass()}
          rows={3}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          disabled={isLoading}
        />
      </div>
    </>
  );
}

function ExceptionForm({
  target,
  isLoading,
  error,
  onClose,
  onConfirm,
}: {
  target: CirculationExceptionTarget;
  isLoading: boolean;
  error: string | null;
  onClose: () => void;
  onConfirm: (payload: CirculationExceptionSubmit) => void;
}) {
  const { t } = useTranslation();
  const [bill, setBill] = useState(false);
  const [amount, setAmount] = useState('');
  const [disposition, setDisposition] = useState<DamageDisposition>('return');
  const [notes, setNotes] = useState('');
  const [inventoryChecked, setInventoryChecked] = useState(false);

  const kind = target.kind;
  const keys = copyKey(kind);
  const needsInventory = kind === 'resolveFound' || kind === 'resolveNotFound';
  const amountInvalid = bill && kind === 'damaged' && !isPositiveAmount(amount);
  const confirmDisabled = (needsInventory && !inventoryChecked) || (kind === 'damaged' && amountInvalid);

  return (
    <ConfirmDialog
      isOpen
      onClose={onClose}
      onConfirm={() => {
        if (confirmDisabled) return;
        onConfirm({ bill, amount, disposition, notes, inventoryChecked });
      }}
      title={t(keys.title)}
      message={t(keys.message)}
      confirmLabel={t(keys.confirm)}
      confirmVariant={kind === 'lost' || kind === 'resolveNotFound' ? 'danger' : 'primary'}
      isLoading={isLoading}
      error={error}
      size="md"
      confirmDisabled={confirmDisabled}
    >
      <ExceptionFields
        target={target}
        isLoading={isLoading}
        bill={bill}
        setBill={setBill}
        amount={amount}
        setAmount={setAmount}
        disposition={disposition}
        setDisposition={setDisposition}
        notes={notes}
        setNotes={setNotes}
        inventoryChecked={inventoryChecked}
        setInventoryChecked={setInventoryChecked}
        amountInvalid={amountInvalid}
      />
    </ConfirmDialog>
  );
}

export default function CirculationExceptionDialog({
  target,
  isLoading,
  error,
  onClose,
  onConfirm,
}: CirculationExceptionDialogProps) {
  if (!target) return null;
  return (
    <ExceptionForm
      key={`${target.loanId}-${target.kind}`}
      target={target}
      isLoading={isLoading}
      error={error}
      onClose={onClose}
      onConfirm={onConfirm}
    />
  );
}
