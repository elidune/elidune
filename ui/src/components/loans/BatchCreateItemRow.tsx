import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import type { BatchCreateResult } from '@/types';
import { batchCreateItemMessage } from '@/utils/batchCreate';
import { checkoutDeskActions } from '@/utils/deskCheckoutError';

interface BatchCreateItemRowProps {
  item: BatchCreateResult;
  /** Loans write. `forceable: true` without it shows the message only. */
  canForce: boolean;
  onForce: (barcode: string) => void;
  /** Check the copy in for another reader's hold. */
  onSetAside?: (barcode: string) => void;
}

/**
 * One failed `POST /loans/batch-create` row.
 * « Forcer le prêt » only when the row's `forceable` is true.
 * A held copy keeps « Mettre de côté » / « Annuler ».
 * A copy already set aside (`item_to_pick_for_other`) has neither action.
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
  const actions = checkoutDeskActions(item.code, item.forceable, canForce);
  return (
    <div data-batch-item={item.barcode} className="flex flex-col items-start gap-2">
      <p className="text-sm font-medium text-red-800 dark:text-red-200">
        {batchCreateItemMessage(item, (key) => String(t(key)))}
      </p>
      {actions.setAside && !asideDismissed && (
        <div className="flex flex-wrap gap-2">
          <Button type="button" size="sm" variant="primary" onClick={() => onSetAside?.(item.barcode)}>
            {t('loans.toPickSetAside')}
          </Button>
          <Button type="button" size="sm" variant="secondary" onClick={() => setAsideDismissed(true)}>
            {t('common.cancel')}
          </Button>
        </div>
      )}
      {actions.force && (
        <CirculationForceButton
          forceable={item.forceable === true}
          canForce={canForce}
          onForce={() => onForce(item.barcode)}
        />
      )}
    </div>
  );
}
