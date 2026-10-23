import { mediaTypeIconBadgeBgClass, renderMediaTypeIcon } from '@/utils/mediaTypeIconUtils';
import type { MediaType } from '@/types';
import { depositLenderInitials, uniqueDeposits, type DepositCopyRef } from '@/utils/deposits';

export type LoanMediaTypeBadgeSize = 'table' | 'catalog' | 'comfortable';

const BADGE_SIZES: Record<LoanMediaTypeBadgeSize, { wrap: string; icon: string }> = {
  table: { wrap: 'h-10 w-10 rounded-lg', icon: 'h-5 w-5' },
  catalog: { wrap: 'h-12 w-12 rounded-lg', icon: 'h-5 w-5' },
  comfortable: { wrap: 'h-16 w-16 rounded-xl', icon: 'h-8 w-8' },
};

const DEPOSIT_OVERLAY_SIZES: Record<
  LoanMediaTypeBadgeSize,
  { wrap: string; initials: string }
> = {
  table: { wrap: 'h-[1.125rem] w-[1.125rem] -bottom-px -right-px', initials: 'text-[9px]' },
  catalog: { wrap: 'h-5 w-5 bottom-0 right-0', initials: 'text-[10px]' },
  comfortable: { wrap: 'h-6 w-6 -bottom-0.5 -right-0.5', initials: 'text-xs' },
};

/** Rounded badge with icon — catalog-style presentation for loan / biblio rows. */
export function LoanMediaTypeBadge({
  mediaType,
  size = 'table',
  className = '',
  depositCopies,
}: {
  mediaType?: MediaType | string | null;
  size?: LoanMediaTypeBadgeSize;
  className?: string;
  /** When set, a lender-initials overlay marks deposit stock on the document icon. */
  depositCopies?: DepositCopyRef[] | null;
}) {
  const { wrap, icon } = BADGE_SIZES[size];
  const deposits = uniqueDeposits(depositCopies);
  const primaryDeposit = deposits[0];
  const overlay = DEPOSIT_OVERLAY_SIZES[size];
  const depositTitle =
    deposits.length > 0
      ? deposits.map((row) => row.lenderName).filter(Boolean).join(' · ')
      : undefined;

  return (
    <div
      className={['relative shrink-0', className].filter(Boolean).join(' ')}
      title={depositTitle}
      aria-label={depositTitle}
    >
      <div
        className={`${wrap} ${mediaTypeIconBadgeBgClass(mediaType)} flex items-center justify-center`}
        aria-hidden={depositTitle ? true : undefined}
      >
        {renderMediaTypeIcon(mediaType, icon)}
      </div>
      {primaryDeposit ? (
        <span
          className={[
            'pointer-events-none absolute flex items-center justify-center rounded-full font-semibold',
            'bg-violet-100 text-violet-800 ring-2 ring-white dark:bg-violet-900/55 dark:text-violet-200 dark:ring-gray-900',
            overlay.wrap,
            overlay.initials,
          ].join(' ')}
          aria-hidden
        >
          {depositLenderInitials(primaryDeposit.lenderName)}
        </span>
      ) : null}
    </div>
  );
}
