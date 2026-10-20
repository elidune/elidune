import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal, Button } from '@/components/common';
import type { DepositDuplicateAction, DepositDuplicateConflict } from '@/types';

export type DepositMarcDuplicateResolution = {
  action: DepositDuplicateAction;
  applyToAll: boolean;
};

type Props = {
  isOpen: boolean;
  conflicts: DepositDuplicateConflict[];
  isLoading?: boolean;
  onCancel: () => void;
  onConfirm: (resolution: DepositMarcDuplicateResolution) => void;
};

function conflictKindKey(kind: DepositDuplicateConflict['kind']): string {
  switch (kind) {
    case 'activeBarcode':
      return 'importMarc.depositDuplicate.kindActiveBarcode';
    case 'reactivation':
      return 'importMarc.depositDuplicate.kindReactivation';
    case 'wasLost':
      return 'importMarc.depositDuplicate.kindWasLost';
    default:
      return 'importMarc.depositDuplicate.kindActiveBarcode';
  }
}

export default function DepositMarcDuplicateDialog({
  isOpen,
  conflicts,
  isLoading = false,
  onCancel,
  onConfirm,
}: Props) {
  const { t } = useTranslation();
  const [action, setAction] = useState<DepositDuplicateAction>('relabel');
  const [applyToAll, setApplyToAll] = useState(conflicts.length > 1);

  const primary = conflicts[0];
  const lostOnly = conflicts.every((c) => c.kind === 'wasLost');

  return (
    <Modal
      isOpen={isOpen}
      onClose={onCancel}
      title={t('importMarc.depositDuplicate.title')}
      footer={
        <>
          <Button variant="ghost" onClick={onCancel} disabled={isLoading}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="secondary"
            disabled={isLoading || lostOnly}
            onClick={() => onConfirm({ action: 'ignore', applyToAll })}
          >
            {t('importMarc.depositDuplicate.ignore')}
          </Button>
          <Button
            isLoading={isLoading}
            disabled={lostOnly}
            onClick={() => onConfirm({ action, applyToAll })}
          >
            {t('importMarc.depositDuplicate.confirm')}
          </Button>
        </>
      }
    >
      <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">{t('importMarc.depositDuplicate.intro')}</p>
      {conflicts.length > 1 ? (
        <p className="text-sm font-medium text-gray-900 dark:text-white mb-3">
          {t('importMarc.depositDuplicate.count', { count: conflicts.length })}
        </p>
      ) : null}
      {primary ? (
        <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-950/40 p-3 mb-4 text-sm">
          <p className="font-medium text-gray-900 dark:text-white">
            {t('importMarc.depositDuplicate.lenderBarcode')}:{' '}
            <span className="font-mono">{primary.lenderBarcode}</span>
          </p>
          <p className="mt-1 text-gray-700 dark:text-gray-300">{t(conflictKindKey(primary.kind))}</p>
          {primary.existingBarcode ? (
            <p className="mt-1 text-gray-600 dark:text-gray-400">
              {t('importMarc.depositDuplicate.existingBarcode')}:{' '}
              <span className="font-mono">{primary.existingBarcode}</span>
            </p>
          ) : null}
        </div>
      ) : null}
      {!lostOnly ? (
        <fieldset className="space-y-2 mb-4">
          <legend className="text-sm font-medium text-gray-900 dark:text-white">
            {t('importMarc.depositDuplicate.actionLegend')}
          </legend>
          <label className="flex items-start gap-2 text-sm cursor-pointer">
            <input
              type="radio"
              name="deposit-dup-action"
              checked={action === 'overwrite'}
              onChange={() => setAction('overwrite')}
              className="mt-1"
            />
            <span>{t('importMarc.depositDuplicate.overwriteHint')}</span>
          </label>
          <label className="flex items-start gap-2 text-sm cursor-pointer">
            <input
              type="radio"
              name="deposit-dup-action"
              checked={action === 'relabel'}
              onChange={() => setAction('relabel')}
              className="mt-1"
            />
            <span>{t('importMarc.depositDuplicate.relabelHint')}</span>
          </label>
        </fieldset>
      ) : (
        <p className="text-sm text-red-600 dark:text-red-400 mb-4">{t('importMarc.depositDuplicate.lostOnly')}</p>
      )}
      {conflicts.length > 1 ? (
        <label className="flex items-center gap-2 text-sm cursor-pointer">
          <input
            type="checkbox"
            checked={applyToAll}
            onChange={(e) => setApplyToAll(e.target.checked)}
          />
          {t('importMarc.depositDuplicate.applyToAll')}
        </label>
      ) : null}
    </Modal>
  );
}
