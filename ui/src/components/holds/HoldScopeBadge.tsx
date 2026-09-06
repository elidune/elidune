import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { Hold } from '@/types';
import { holdScopeKind } from '@/utils/holdDisplay';

export default function HoldScopeBadge({ hold }: { hold: Hold }) {
  const { t } = useTranslation();
  const kind = holdScopeKind(hold);
  if (kind === 'title') {
    return <Badge size="sm" variant="info">{t('holds.scopeTitleShort')}</Badge>;
  }
  if (kind === 'pinned') {
    return <Badge size="sm" variant="warning">{t('holds.scopePinnedShort')}</Badge>;
  }
  return <Badge size="sm">{t('holds.scopeAllocatedShort')}</Badge>;
}
