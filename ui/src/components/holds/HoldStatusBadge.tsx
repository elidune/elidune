import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { HoldStatus } from '@/types';

export type HoldBadgeStatus = HoldStatus | 'inTransit';

function variantFor(status: HoldBadgeStatus): 'default' | 'success' | 'warning' | 'info' {
  if (status === 'ready') return 'success';
  if (status === 'to_pick' || status === 'inTransit') return 'info';
  if (status === 'expired') return 'warning';
  return 'default';
}

/** Shared hold-status pill (holds list, patron holds, user file). */
export default function HoldStatusBadge({ status }: { status: HoldBadgeStatus }) {
  const { t } = useTranslation();
  return <Badge variant={variantFor(status)}>{t(`holds.statuses.${status}`)}</Badge>;
}
