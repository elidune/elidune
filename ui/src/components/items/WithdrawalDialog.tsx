import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useMutation } from '@tanstack/react-query';
import { Button, Modal } from '@/components/common';
import api from '@/services/api';
import type { WithdrawalDestination, WithdrawalReason } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import {
  WITHDRAWAL_DESTINATIONS,
  WITHDRAWAL_DETAIL_MIN,
  WITHDRAWAL_REASONS,
  validateWithdrawalWrite,
  type WithdrawalFieldError,
} from '@/utils/withdrawals';

interface WithdrawalDialogProps {
  open: boolean;
  mode: 'create' | 'complete';
  /** Copies to withdraw. Ignored when `mode` is `complete`. */
  itemIds: string[];
  withdrawalId?: string | null;
  initialReason?: WithdrawalReason | '';
  initialDetail?: string;
  initialDestination?: WithdrawalDestination | '';
  onClose: () => void;
  onSuccess: () => void;
}

export default function WithdrawalDialog({
  open,
  mode,
  itemIds,
  withdrawalId,
  initialReason = '',
  initialDetail = '',
  initialDestination = '',
  onClose,
  onSuccess,
}: WithdrawalDialogProps) {
  const { t } = useTranslation();
  const [reason, setReason] = useState<WithdrawalReason | ''>(initialReason);
  const [reasonDetail, setReasonDetail] = useState(initialDetail);
  const [destination, setDestination] = useState<WithdrawalDestination | ''>(initialDestination);
  const [fieldError, setFieldError] = useState<WithdrawalFieldError | null>(null);
  const [formError, setFormError] = useState<string | null>(null);
  const [seed, setSeed] = useState({ open, initialReason, initialDetail, initialDestination });

  if (
    seed.open !== open ||
    seed.initialReason !== initialReason ||
    seed.initialDetail !== initialDetail ||
    seed.initialDestination !== initialDestination
  ) {
    setSeed({ open, initialReason, initialDetail, initialDestination });
    if (open) {
      setReason(initialReason);
      setReasonDetail(initialDetail);
      setDestination(initialDestination);
      setFieldError(null);
      setFormError(null);
    }
  }

  const mutation = useMutation({
    mutationFn: async () => {
      const parsed = validateWithdrawalWrite({ reason, reasonDetail, destination });
      if (!parsed.ok) {
        setFieldError(parsed.field);
        throw new Error('invalid');
      }
      setFieldError(null);
      if (mode === 'complete') {
        if (!withdrawalId) throw new Error('missing withdrawal');
        return api.completeLegacyWithdrawal(withdrawalId, parsed.body);
      }
      if (itemIds.length === 1) {
        return api.withdrawItem(itemIds[0], parsed.body);
      }
      return api.withdrawItems({ itemIds, ...parsed.body });
    },
    onSuccess: () => {
      onSuccess();
      onClose();
    },
    onError: (err) => {
      if (err instanceof Error && (err.message === 'invalid' || err.message === 'missing withdrawal')) {
        return;
      }
      setFormError(getApiErrorMessage(err, t));
    },
  });

  const detailRequired = reason === 'other';
  const title =
    mode === 'complete'
      ? t('weeding.completeTitle')
      : itemIds.length > 1
        ? t('weeding.dialogTitleBatch', { count: itemIds.length })
        : t('weeding.dialogTitle');

  return (
    <Modal
      isOpen={open}
      onClose={() => {
        if (mutation.isPending) return;
        onClose();
      }}
      title={title}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="secondary" disabled={mutation.isPending} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="danger"
            isLoading={mutation.isPending}
            disabled={mode === 'create' && itemIds.length === 0}
            onClick={() => {
              setFormError(null);
              mutation.mutate();
            }}
          >
            {mode === 'complete' ? t('common.save') : t('weeding.confirmWithdraw')}
          </Button>
        </div>
      }
    >
      <div className="space-y-4">
        {mode === 'complete' ? (
          <p className="text-sm text-gray-600 dark:text-gray-300">{t('weeding.legacyHint')}</p>
        ) : (
          <p className="text-sm text-gray-600 dark:text-gray-300">{t('weeding.dialogHint')}</p>
        )}

        <div>
          <label htmlFor="withdrawal-reason" className={formLabelClass()}>
            {t('weeding.reason')}
            <span className="text-red-600 dark:text-red-400 ml-0.5" aria-hidden="true">*</span>
          </label>
          <select
            id="withdrawal-reason"
            className={formControlClass({ error: fieldError === 'reason', className: 'w-full' })}
            value={reason}
            onChange={(e) => {
              setReason(e.target.value as WithdrawalReason | '');
              setFieldError(null);
            }}
          >
            <option value="">{t('weeding.reasonPlaceholderSelect')}</option>
            {WITHDRAWAL_REASONS.map((value) => (
              <option key={value} value={value}>
                {t(`weeding.reasons.${value}`)}
              </option>
            ))}
          </select>
          {fieldError === 'reason' ? (
            <p className="mt-1 text-xs text-red-600 dark:text-red-400">{t('weeding.reasonRequired')}</p>
          ) : null}
        </div>

        <div>
          <label htmlFor="withdrawal-detail" className={formLabelClass()}>
            {t('weeding.detailLabel')}
            {detailRequired ? (
              <span className="text-red-600 dark:text-red-400 ml-0.5" aria-hidden="true">*</span>
            ) : null}
          </label>
          <textarea
            id="withdrawal-detail"
            className={formControlClass({
              error: fieldError === 'detail',
              autoHeight: true,
              className: 'w-full min-h-20 py-2',
            })}
            value={reasonDetail}
            maxLength={2000}
            required={detailRequired}
            onChange={(e) => {
              setReasonDetail(e.target.value);
              setFieldError(null);
            }}
          />
          <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
            {detailRequired
              ? t('weeding.detailRequired', { count: WITHDRAWAL_DETAIL_MIN })
              : t('weeding.detailOptional')}
          </p>
          {fieldError === 'detail' ? (
            <p className="mt-1 text-xs text-red-600 dark:text-red-400">
              {t('weeding.detailTooShort', { count: WITHDRAWAL_DETAIL_MIN })}
            </p>
          ) : null}
        </div>

        <div>
          <label htmlFor="withdrawal-destination" className={formLabelClass()}>
            {t('weeding.destinationLabel')}
            <span className="text-red-600 dark:text-red-400 ml-0.5" aria-hidden="true">*</span>
          </label>
          <select
            id="withdrawal-destination"
            className={formControlClass({ error: fieldError === 'destination', className: 'w-full' })}
            value={destination}
            onChange={(e) => {
              setDestination(e.target.value as WithdrawalDestination | '');
              setFieldError(null);
            }}
          >
            <option value="">{t('weeding.destinationPlaceholder')}</option>
            {WITHDRAWAL_DESTINATIONS.map((value) => (
              <option key={value} value={value}>
                {t(`weeding.destinations.${value}`)}
              </option>
            ))}
          </select>
          {fieldError === 'destination' ? (
            <p className="mt-1 text-xs text-red-600 dark:text-red-400">{t('weeding.destinationRequired')}</p>
          ) : null}
        </div>

        {formError ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {formError}
          </p>
        ) : null}
      </div>
    </Modal>
  );
}
