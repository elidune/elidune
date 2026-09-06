import type { ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Ban } from 'lucide-react';
import { Button } from '@/components/common';
import HoldDocumentCell from '@/components/holds/HoldDocumentCell';
import HoldExpiresCell from '@/components/holds/HoldExpiresCell';
import HoldPickupCell from '@/components/holds/HoldPickupCell';
import type { Hold, ItemTransit, Source } from '@/types';
import { formatUserShortName } from '@/utils/userDisplay';

interface HoldMobileCardProps {
  hold: Hold;
  statusBadge: (status: Hold['status']) => ReactNode;
  onCancel: () => void;
  cancelPending: boolean;
  /** When false, omit patron name (self-service list). Default true. */
  showUser?: boolean;
  /** Highlight pickup expiry for ready holds (patron UX). */
  emphasizePickup?: boolean;
  pickupSources?: Source[];
  transit?: ItemTransit | null;
  extraActions?: ReactNode;
}

export default function HoldMobileCard({
  hold,
  statusBadge,
  onCancel,
  cancelPending,
  showUser = true,
  emphasizePickup = false,
  pickupSources,
  transit,
  extraActions,
}: HoldMobileCardProps) {
  const { t, i18n } = useTranslation();
  const userLabel = formatUserShortName(hold.user) || hold.userId;
  const showCancel = hold.status === 'pending' || hold.status === 'ready';

  return (
    <div className="p-4 border-b border-gray-100 dark:border-gray-800 last:border-b-0 space-y-2">
      <div className="flex items-start justify-between gap-2">
        {showUser ? (
          <Link
            className="text-sm font-medium text-indigo-600 dark:text-indigo-400 hover:underline min-w-0"
            to={`/users/${hold.userId}`}
          >
            {userLabel}
          </Link>
        ) : (
          <div className="text-sm min-w-0 flex-1">
            <HoldDocumentCell hold={hold} />
          </div>
        )}
        {statusBadge(hold.status)}
      </div>
      {showUser && (
        <div className="text-sm">
          <HoldDocumentCell hold={hold} />
        </div>
      )}
      <div className="grid grid-cols-2 gap-2 text-xs text-gray-600 dark:text-gray-300">
        <div>
          <span className="text-gray-500 block">{t('holds.position')}</span>
          {t('holds.queuePosition', { position: hold.position })}
        </div>
        <div>
          <span className="text-gray-500 block">{t('holds.createdAt')}</span>
          {new Date(hold.createdAt).toLocaleString(i18n.language)}
        </div>
        <div className="col-span-2">
          <HoldExpiresCell hold={hold} emphasizePickup={emphasizePickup} showLabel />
        </div>
        {(hold.pickupSiteId || transit) && (
          <div className="col-span-2">
            <span className="text-gray-500 block">{t('holds.columnPickup')}</span>
            <HoldPickupCell hold={hold} sources={pickupSources} transit={transit} />
          </div>
        )}
      </div>
      {(extraActions || showCancel) && (
        <div className="flex flex-wrap gap-2">
          {extraActions}
          {showCancel && (
        <Button
          size="sm"
          variant="secondary"
          leftIcon={<Ban className="h-4 w-4" />}
          isLoading={cancelPending}
          onClick={onCancel}
        >
            {t('holds.cancelHold')}
          </Button>
          )}
        </div>
      )}
    </div>
  );
}
