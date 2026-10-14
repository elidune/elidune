import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import type { BatchCreateResult } from '@/types';
import { batchCreateItemMessage } from '@/utils/batchCreate';

interface BatchCreateItemRowProps {
  item: BatchCreateResult;
  /** Loans write. `forceable: true` without it shows the message only. */
  canForce: boolean;
  onForce: (barcode: string) => void;
  /** Check the copy in for another reader's hold. */
  onSetAside?: (barcode: string) => void;
}

const HELD_FOR_OTHER = 'item_held_for_other';
const STATUS_ACTION = 'item_status_action_required';

/**
 * One failed `POST /loans/batch-create` row.
 * « Forcer le prêt » only when the row's `forceable` is true.
 * A held copy keeps « Mettre de côté » / « Annuler ». A status action has no force button.
 */
export default function BatchCreateItemRow({
  item,
  canForce,
  onForce,
  onSetAside,
}: BatchCreateItemRowProps) {
  const { t } = useTranslation();
  const [asideDismissed, setAsideDismissed] = useState(false);
  if (item.success) return null;
  const code = item.code?.trim() ?? '';
  const held = code === HELD_FOR_OTHER;
  const statusAction = code === STATUS_ACTION;
  return (
    <div data-batch-item={item.barcode} className="flex flex-col items-start gap-2">
      <p className="text-sm font-medium text-red-800 dark:text-red-200">
        {batchCreateItemMessage(item, (key) => String(t(key)))}
      </p>
      {held && !asideDismissed && (
        <div className="flex flex-wrap gap-2">
          <Button type="button" size="sm" variant="primary" onClick={() => onSetAside?.(item.barcode)}>
            {t('loans.toPickSetAside')}
          </Button>
          <Button type="button" size="sm" variant="secondary" onClick={() => setAsideDismissed(true)}>
            {t('common.cancel')}
          </Button>
        </div>
      )}
      {!held && !statusAction && (
        <CirculationForceButton
          forceable={item.forceable === true}
          canForce={canForce}
          onForce={() => onForce(item.barcode)}
        />
      )}
    </div>
  );
}
