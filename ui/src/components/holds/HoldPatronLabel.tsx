import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import type { BorrowerRef } from '@/utils/readerIdentifier';

interface HoldPatronLabelProps {
  user?: BorrowerRef | null;
  /** Link to the reader record. Pass only when the caller can read users. */
  linked?: boolean;
  className?: string;
}

/**
 * Reader on a staff hold list.
 * Full profile: name. Minimized profile: card number or initials.
 */
export default function HoldPatronLabel({ user, linked = false, className }: HoldPatronLabelProps) {
  return (
    <ReaderIdentifier
      user={user}
      linked={linked}
      cardLabelKey="holds.readerCardNo"
      className={className}
    />
  );
}
