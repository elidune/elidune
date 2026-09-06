import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { TransitStatus } from '@/types';

const VARIANT: Record<TransitStatus, 'default' | 'success' | 'warning' | 'info'> = {
  requested: 'warning',
  inTransit: 'info',
  received: 'success',
  cancelled: 'default',
};

export default function TransitStatusBadge({ status }: { status: TransitStatus }) {
  const { t } = useTranslation();
  return (
    <Badge size="sm" variant={VARIANT[status]}>
      {t(`transits.statuses.${status}`)}
    </Badge>
  );
}
