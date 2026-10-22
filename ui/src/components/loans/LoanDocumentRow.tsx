import type { ReactNode } from 'react';
import type { MediaType } from '@/types';
import { LoanMediaTypeBadge, type LoanMediaTypeBadgeSize } from '@/utils/mediaTypeIcon';

interface LoanDocumentRowProps {
  mediaType?: MediaType | string | null;
  size?: LoanMediaTypeBadgeSize;
  title: ReactNode;
  /** Metadata under the title row (barcode, identity, ISBN). */
  details?: ReactNode;
}

/** Media-type icon aligned with the title on one row; optional details below. */
export default function LoanDocumentRow({
  mediaType,
  size = 'table',
  title,
  details,
}: LoanDocumentRowProps) {
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-3 min-w-0">
        <LoanMediaTypeBadge mediaType={mediaType} size={size} />
        <div className="min-w-0 flex-1">{title}</div>
      </div>
      {details ? <div className="mt-1 min-w-0">{details}</div> : null}
    </div>
  );
}
