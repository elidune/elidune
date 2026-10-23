import { useRef, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import { Card, BarcodeScanField } from '@/components/common';
import api from '@/services/api';
import type { DepositGlobalScanOut } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import { depositScanAlreadyReturned } from '@/utils/deposits';

const BATCHES_KEY = ['deposit-batches'] as const;

type DepositGlobalReturnScanProps = {
  canWrite: boolean;
};

export default function DepositGlobalReturnScan({ canWrite }: DepositGlobalReturnScanProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const scanRef = useRef<HTMLInputElement>(null);
  const [barcode, setBarcode] = useState('');
  const [notice, setNotice] = useState<string | null>(null);
  const [lastBatchId, setLastBatchId] = useState<string | null>(null);

  const scanMutation = useMutation({
    mutationFn: (code: string) => api.scanOutDepositCopyGlobal(code),
    onSuccess: (body: DepositGlobalScanOut, code) => {
      const already = depositScanAlreadyReturned(body, false);
      setNotice(
        already
          ? t('deposits.globalScan.alreadyScanned', {
              barcode: code,
              batch: body.batchReference,
              lender: body.lenderName,
            })
          : t('deposits.globalScan.scanned', {
              barcode: code,
              batch: body.batchReference,
              lender: body.lenderName,
            }),
      );
      setLastBatchId(body.depositBatchId);
      setBarcode('');
      void queryClient.invalidateQueries({ queryKey: BATCHES_KEY });
      void queryClient.invalidateQueries({ queryKey: ['deposit-batch', body.depositBatchId] });
      requestAnimationFrame(() => scanRef.current?.focus());
    },
    onError: (error: unknown) => {
      setLastBatchId(null);
      setNotice(getApiErrorMessage(error, t));
      requestAnimationFrame(() => {
        scanRef.current?.focus();
        scanRef.current?.select();
      });
    },
  });

  if (!canWrite) return null;

  return (
    <Card>
      <div className="border-b border-gray-200 px-4 py-3 dark:border-gray-700">
        <h2 className="text-base font-semibold text-gray-900 dark:text-white">{t('deposits.globalScan.title')}</h2>
        <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">{t('deposits.globalScan.hint')}</p>
      </div>
      <form
        className="p-4"
        onSubmit={(event) => {
          event.preventDefault();
          const code = barcode.trim();
          if (!code || scanMutation.isPending) return;
          scanMutation.mutate(code);
        }}
      >
        <BarcodeScanField
          ref={scanRef}
          label={t('items.barcode')}
          value={barcode}
          autoFocus
          onChange={(event) => setBarcode(event.target.value)}
          onCameraScan={(code) => {
            setBarcode(code);
            if (code.trim() && !scanMutation.isPending) scanMutation.mutate(code.trim());
          }}
          scannerTitle={t('loans.scanOrEnterBarcode')}
        />
      </form>
      {notice ? (
        <p className="px-4 pb-4 text-sm text-gray-700 dark:text-gray-300" role="status">
          {notice}
          {lastBatchId ? (
            <>
              {' '}
              <Link
                to={`/deposits/${lastBatchId}`}
                className="font-medium text-blue-700 underline dark:text-blue-300"
              >
                {t('deposits.globalScan.openBatch')}
              </Link>
            </>
          ) : null}
        </p>
      ) : null}
    </Card>
  );
}
