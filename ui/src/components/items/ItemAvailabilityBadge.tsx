import { useTranslation } from 'react-i18next';
import { Badge } from '@/components/common';
import PatronLink from '@/components/users/PatronLink';
import {
  availabilityBadgeVariant,
  availabilityLabelKey,
  normalizeAvailability,
  type AvailabilityAudience,
} from '@/utils/itemAvailability';

interface ItemAvailabilityBadgeProps {
  availability?: string | null;
  audience?: AvailabilityAudience;
  size?: 'sm' | 'md';
}

/** Shared copy-status pill. Label and color come from the server `availability` value. */
export default function ItemAvailabilityBadge({
  availability,
  audience = 'staff',
  size = 'md',
}: ItemAvailabilityBadgeProps) {
  const { t } = useTranslation();
  const status = normalizeAvailability(availability, audience);
  return (
    <Badge variant={availabilityBadgeVariant(status)} size={size}>
      {t(availabilityLabelKey(status))}
    </Badge>
  );
}

interface ItemHoldPatronLineProps {
  audience: AvailabilityAudience;
  availability?: string | null;
  holdUserId?: string | null;
  holdUserName?: string | null;
}

/** Staff only. Readers and the OPAC never receive the patron, and this renders nothing for them. */
export function ItemHoldPatronLine({
  audience,
  availability,
  holdUserId,
  holdUserName,
}: ItemHoldPatronLineProps) {
  const { t } = useTranslation();
  if (audience !== 'staff') return null;
  if (normalizeAvailability(availability, 'staff') !== 'onHold') return null;
  const name = holdUserName?.trim();
  if (!name && !holdUserId) return null;
  const label = t('availability.reservedFor', { name: name || holdUserId });
  return (
    <p className="text-sm">
      <PatronLink
        userId={holdUserId}
        className="text-indigo-600 dark:text-indigo-400 hover:underline"
      >
        {label}
      </PatronLink>
    </p>
  );
}
