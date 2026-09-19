import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { PurchaseSuggestionStatus } from '@/types';
import { suggestionStatusBadgeVariant } from '@/utils/acquisitionDisplay';

export default function SuggestionStatusBadge({ status }: { status: PurchaseSuggestionStatus }) {
  const { t } = useTranslation();
  return (
    <Badge variant={suggestionStatusBadgeVariant(status)} size="sm">
      {t(`acquisitions.suggestionStatuses.${status}`)}
    </Badge>
  );
}
