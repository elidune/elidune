import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import CirculationForceButton from '@/components/loans/CirculationForceButton';
import type { BatchCreateResult } from '@/types';
import { batchCreateItemMessage } from '@/utils/batchCreate';
import { checkoutDeskActions } from '@/utils/deskCheckoutError';
import { showDeskToast } from '@/utils/deskToast';
import {
  commitDismissHeldSetAside,
  commitHeldSetAside,
  heldSetAsideServerMessage,
} from '@/utils/heldSetAside';

interface BatchCreateItemRowProps {
  item: BatchCreateResult;
  /** Loans write. `forceable: true` without it shows the message only. */
  canForce: boolean;
  onForce: (barcode: string) => void;
  /**
   * Unused. Set-aside runs in place on this row (`POST /loans/items/{barcode}/return`)
   * and does not leave checkout. Kept so callers can still pass it.
   */
  onSetAside?: (barcode: string) => void;
  /** Clear (`true`) or only refocus (`false`) the checkout barcode. */
  onBarcodeCue?: (clear: boolean) => void;
}

/**
 * One failed `POST /loans/batch-create` row.
 * « Forcer le prêt » only when the row's `forceable` is true.
 * This route does not accept `overrideReason`, so `reasonRequired: true`
 * offers no force button and no reason field.
 * A held copy keeps « Mettre de côté » / « Annuler ».
 * A copy already set aside (`item_to_pick_for_other`) has neither action.
 */
export default function BatchCreateItemRow({
  item,
  canForce,
  onForce,
  onBarcodeCue,
}: BatchCreateItemRowProps) {
  const { t } = useTranslation();
  const [asideDismissed, setAsideDismissed] = useState(false);
  const [asidePending, setAsidePending] = useState(false);
  if (item.success) return null;
  const actions = checkoutDeskActions(item.code, item.forceable, canForce);
  const offerForce = actions.force && item.reasonRequired !== true;
  return (
    <div data-batch-item={item.barcode} className="flex flex-col items-start gap-2">
      <p className="text-sm font-medium text-red-800 dark:text-red-200">
        {batchCreateItemMessage(item, (key) => String(t(key)))}
      </p>
      {actions.setAside && !asideDismissed && (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            size="sm"
            variant="primary"
            isLoading={asidePending}
            onClick={() => {
              if (asidePending) return;
              setAsidePending(true);
              let clear = false;
              let refocus = false;
              void commitHeldSetAside(item.barcode, {
                returnByBarcode: async (barcode) => {
                  const { default: client } = await import('@/services/api');
                  return client.returnLoanByBarcode(barcode);
                },
                successMessage: t('loans.heldSetAside'),
                errorMessage: (error) => heldSetAsideServerMessage(error, t),
                showToast: (next) => showDeskToast(next),
                closeDialog: () => setAsideDismissed(true),
                clearBarcode: () => {
                  clear = true;
                },
                refocusBarcode: () => {
                  refocus = true;
                },
              }).finally(() => {
                setAsidePending(false);
                if (clear || refocus) onBarcodeCue?.(clear);
              });
            }}
          >
            {t('loans.toPickSetAside')}
          </Button>
          <Button
            type="button"
            size="sm"
            variant="secondary"
            onClick={() => {
              let refocus = false;
              commitDismissHeldSetAside({
                closeDialog: () => setAsideDismissed(true),
                clearBarcode: () => undefined,
                refocusBarcode: () => {
                  refocus = true;
                },
              });
              if (refocus) onBarcodeCue?.(false);
            }}
          >
            {t('common.cancel')}
          </Button>
          {offerForce && (
            <CirculationForceButton
              forceable={item.forceable === true}
              canForce={canForce}
              onForce={() => onForce(item.barcode)}
              disabled={asidePending}
            />
          )}
        </div>
      )}
      {offerForce && !actions.setAside && (
        <CirculationForceButton
          forceable={item.forceable === true}
          canForce={canForce}
          onForce={() => onForce(item.barcode)}
        />
      )}
    </div>
  );
}
