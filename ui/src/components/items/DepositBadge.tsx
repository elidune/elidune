import { ExternalLink } from 'lucide-react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import Badge from '@/components/common/Badge';
import type { ItemDeposit, ItemDepositPublic } from '@/types';
import type { AvailabilityAudience } from '@/utils/itemAvailability';
import {
  depositBadgeText,
  depositBadgeTooltip,
  depositLossNotifiedText,
  isDepositCopy,
  uniqueDeposits,
  type DepositCopyRef,
} from '@/utils/deposits';

interface DepositBadgeProps {
  deposit?: ItemDeposit | ItemDepositPublic | null;
  size?: 'sm' | 'md';
  audience?: AvailabilityAudience;
}

/** Sober pill. Own stock (`deposit` null) renders nothing. Staff: tooltip + link to the batch. */
export default function DepositBadge({ deposit, size = 'sm', audience = 'staff' }: DepositBadgeProps) {
  const { t, i18n } = useTranslation();
  const label = depositBadgeText(deposit, t, i18n.language, audience);
  if (!label) return null;
  const tooltip = depositBadgeTooltip(deposit, t, i18n.language, audience);
  const linkable = audience === 'staff' && isDepositCopy(deposit);
  const pill = (
    <Badge variant="default" size={size}>
      <span className="inline-flex max-w-full items-center gap-1">
        <span className="truncate">{label}</span>
        {linkable ? <ExternalLink className="h-3 w-3 shrink-0 opacity-70" aria-hidden /> : null}
      </span>
    </Badge>
  );
  if (linkable) {
    return (
      <Link
        to={`/deposits/${deposit.batchId}`}
        title={tooltip ?? undefined}
        aria-label={t('deposits.badgeOpen', { lender: deposit.lenderName, reference: deposit.reference })}
        className="inline-flex max-w-full rounded-full hover:opacity-90 focus:outline-none focus-visible:ring-2 focus-visible:ring-amber-500"
        onClick={(event) => event.stopPropagation()}
      >
        {pill}
      </Link>
    );
  }
  return <span className="inline-flex max-w-full">{pill}</span>;
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
