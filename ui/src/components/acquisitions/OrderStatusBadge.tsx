import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { PurchaseOrderStatus } from '@/types';
import { orderStatusBadgeVariant } from '@/utils/acquisitionDisplay';

export default function OrderStatusBadge({ status }: { status: PurchaseOrderStatus }) {
  const { t } = useTranslation();
  return (
    <Badge variant={orderStatusBadgeVariant(status)} size="sm">
      {t(`acquisitions.statuses.${status}`)}
    </Badge>
  );
}
