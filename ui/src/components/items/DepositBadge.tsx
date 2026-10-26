import { useTranslation } from 'react-i18next';
import Badge from '@/components/common/Badge';
import type { ItemDeposit, ItemDepositPublic } from '@/types';
import type { AvailabilityAudience } from '@/utils/itemAvailability';
import { depositBadgeText, depositLossNotifiedText, uniqueDeposits, type DepositCopyRef } from '@/utils/deposits';

interface DepositBadgeProps {
  deposit?: ItemDeposit | ItemDepositPublic | null;
  size?: 'sm' | 'md';
  audience?: AvailabilityAudience;
}

/** Sober pill. Own stock (`deposit` null) renders nothing. */
export default function DepositBadge({ deposit, size = 'sm', audience = 'staff' }: DepositBadgeProps) {
  const { t, i18n } = useTranslation();
  const label = depositBadgeText(deposit, t, i18n.language, audience);
  if (!label) return null;
  return (
    <Badge variant="default" size={size}>
      {label}
    </Badge>
  );
}

/** Staff-only line. Own stock and copies not yet reported render nothing. */
export function DepositLossNotifiedLine({ deposit }: { deposit?: ItemDeposit | null }) {
  const { t, i18n } = useTranslation();
  const text = depositLossNotifiedText(deposit, t, i18n.language);
  if (!text) return null;
  return <p className="mt-1 text-xs text-gray-600 dark:text-gray-300">{text}</p>;
}

export function CopyDepositBadges({
  copies,
  size = 'sm',
  className = 'mt-1 flex-wrap',
  audience = 'staff',
}: {
  copies?: DepositCopyRef[] | null;
  size?: 'sm' | 'md';
  className?: string;
  audience?: AvailabilityAudience;
}) {
  const deposits = uniqueDeposits(copies);
  if (deposits.length === 0) return null;
  return (
    <div className={['flex gap-1', className].filter(Boolean).join(' ')}>
      {deposits.map((deposit) => (
        <DepositBadge key={deposit.batchId} deposit={deposit} size={size} audience={audience} />
      ))}
    </div>
  );
}
