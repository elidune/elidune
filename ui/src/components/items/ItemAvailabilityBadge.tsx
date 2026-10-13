import { useTranslation } from 'react-i18next';
import Badge from '@/components/common/Badge';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import {
  availabilityBadgeVariant,
  availabilityLabelKey,
  normalizeAvailability,
  type AvailabilityAudience,
} from '@/utils/itemAvailability';
import { readerIdentifierText, type BorrowerRef } from '@/utils/readerIdentifier';

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
  /** Full staff profile, or `{ id, cardNumber, initials }`. Ignored for readers and the OPAC. */
  holdFor?: BorrowerRef | null;
  /** Link to the reader record. Pass only when the caller can read users. */
  linked?: boolean;
}

/**
 * Staff only. The name is shown when `holdFor` is the full profile.
 * Loans-only staff see the card number or initials.
 * Readers and the OPAC render nothing, even if a patron object is passed.
 */
export function ItemHoldPatronLine({
  audience,
  availability,
  holdFor,
  linked = false,
}: ItemHoldPatronLineProps) {
  const { t } = useTranslation();
  if (audience !== 'staff') return null;
  if (normalizeAvailability(availability, 'staff') !== 'onHold') return null;
  if (!holdFor) return null;
  if (readerIdentifierText(holdFor, (number) => number) === '—') return null;
  return (
    <p className="text-sm text-gray-600 dark:text-gray-300">
      {t('availability.reservedFor')}{' '}
      <ReaderIdentifier
        user={holdFor}
        linked={linked}
        cardLabelKey="holds.readerCardNo"
        className="text-indigo-600 dark:text-indigo-400 hover:underline"
      />
    </p>
  );
}
