import { useTranslation } from 'react-i18next';
import Badge from '@/components/common/Badge';
import type { ItemDeposit } from '@/types';
import { depositBadgeText, uniqueDeposits, type DepositCopyRef } from '@/utils/deposits';

interface DepositBadgeProps {
  deposit?: ItemDeposit | null;
  size?: 'sm' | 'md';
}

/** Sober pill. Own stock (`deposit` null) renders nothing. */
export default function DepositBadge({ deposit, size = 'sm' }: DepositBadgeProps) {
  const { t, i18n } = useTranslation();
  const label = depositBadgeText(deposit, t, i18n.language);
  if (!label) return null;
  return (
    <Badge variant="default" size={size}>
      {label}
    </Badge>
  );
}

export function CopyDepositBadges({
  copies,
  size = 'sm',
  className = 'mt-1 flex-wrap',
}: {
  copies?: DepositCopyRef[] | null;
  size?: 'sm' | 'md';
  className?: string;
}) {
  const deposits = uniqueDeposits(copies);
  if (deposits.length === 0) return null;
  return (
    <div className={['flex gap-1', className].filter(Boolean).join(' ')}>
      {deposits.map((deposit) => (
        <DepositBadge key={deposit.batchId} deposit={deposit} size={size} />
      ))}
    </div>
  );
}
