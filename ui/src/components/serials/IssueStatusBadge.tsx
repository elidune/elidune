import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import type { SerialIssueStatus, SerialSubscriptionStatus } from '@/types';
import { issueStatusKey, issueStatusVariant, subscriptionStatusKey } from '@/utils/serialSubscriptionDisplay';

export function IssueStatusBadge({ status }: { status: SerialIssueStatus }) {
  const { t } = useTranslation();
  return (
    <Badge variant={issueStatusVariant(status)} size="sm">
      {t(issueStatusKey(status))}
    </Badge>
  );
}

export function SubscriptionStatusBadge({ status }: { status: SerialSubscriptionStatus }) {
  const { t } = useTranslation();
  const variant = status === 'active' ? 'success' : status === 'expired' ? 'warning' : 'default';
  return (
    <Badge variant={variant} size="sm">
      {t(subscriptionStatusKey(status))}
    </Badge>
  );
}
