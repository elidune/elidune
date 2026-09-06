import { mediaTypeIconBadgeBgClass, renderMediaTypeIcon } from '@/utils/mediaTypeIconUtils';
import type { MediaType } from '@/types';

export type LoanMediaTypeBadgeSize = 'table' | 'catalog' | 'comfortable';

const BADGE_SIZES: Record<LoanMediaTypeBadgeSize, { wrap: string; icon: string }> = {
  table: { wrap: 'h-10 w-10 rounded-lg', icon: 'h-5 w-5' },
  catalog: { wrap: 'h-12 w-12 rounded-lg', icon: 'h-5 w-5' },
  comfortable: { wrap: 'h-14 w-14 rounded-xl', icon: 'h-7 w-7' },
};

/** Rounded badge with icon — catalog-style presentation for loan / biblio rows. */
export function LoanMediaTypeBadge({
  mediaType,
  size = 'table',
  className = '',
}: {
  mediaType?: MediaType | string | null;
  size?: LoanMediaTypeBadgeSize;
  className?: string;
}) {
  const { wrap, icon } = BADGE_SIZES[size];
  return (
    <div
      className={`flex-shrink-0 ${wrap} ${mediaTypeIconBadgeBgClass(mediaType)} flex items-center justify-center ${className}`}
      aria-hidden
    >
      {renderMediaTypeIcon(mediaType, icon)}
    </div>
  );
}
