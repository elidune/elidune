import { useTranslation } from 'react-i18next';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import type { BatchCreateResult } from '@/types';
import { batchCreateItemMessage } from '@/utils/batchCreate';

interface BatchCreateItemRowProps {
  item: BatchCreateResult;
  /** Loans write. A forceable code without it shows the message only. */
  canForce: boolean;
  onForce: (barcode: string) => void;
}

/** One failed `POST /loans/batch-create` row. Only forceable codes show « Forcer le prêt ». */
export default function BatchCreateItemRow({ item, canForce, onForce }: BatchCreateItemRowProps) {
  const { t } = useTranslation();
  if (item.success) return null;
  return (
    <div data-batch-item={item.barcode} className="flex flex-col items-start gap-2">
      <p className="text-sm font-medium text-red-800 dark:text-red-200">
        {batchCreateItemMessage(item, (key) => String(t(key)))}
      </p>
      <CirculationForceButton
        surface="batch"
        code={item.code}
        canForce={canForce}
        onForce={() => onForce(item.barcode)}
      />
    </div>
  );
}
