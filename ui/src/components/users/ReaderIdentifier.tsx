import type { MouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import { patronId } from '@/utils/patronLink';
import {
  borrowerContact,
  readerIdentifierText,
  type BorrowerRef,
} from '@/utils/readerIdentifier';

interface ReaderIdentifierProps {
  user: BorrowerRef | null | undefined;
  /** Link to the reader record. Callers pass this only with users read rights. */
  linked?: boolean;
  /** Overdue full shape: email and phone, each omitted when null or blank. */
  showContact?: boolean;
  /**
   * i18n key for a card phrase, `{{number}}` included.
   * Hold surfaces use `holds.readerCardNo`. Loans keep `loans.readerCardNo`.
   */
  cardLabelKey?: string;
  className?: string;
  onClick?: (event: MouseEvent<HTMLAnchorElement>) => void;
  aside?: ReactNode;
}

/**
 * Reader shown on the desk. A library card is « carte n° », never an item barcode.
 * Without a card, minimized payloads show server initials. The full name is shown
 * only when the payload carries it.
 */
export default function ReaderIdentifier({
  user,
  linked = false,
  showContact = false,
  cardLabelKey = 'loans.readerCardNo',
  className,
  onClick,
  aside,
}: ReaderIdentifierProps) {
  const { t } = useTranslation();
  const label = readerIdentifierText(user, (number) => t(cardLabelKey, { number }));
  const id = linked ? patronId(user?.id) : null;
  const contact = showContact ? borrowerContact(user) : { email: null, phone: null };
  const name = id ? (
    <Link to={`/users/${id}`} className={className} onClick={onClick}>
      {label}
    </Link>
  ) : (
    <span className={className}>{label}</span>
  );

  return (
    <span className="min-w-0">
      <span className="inline-flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
        {name}
        {aside}
      </span>
      {contact.email ? (
        <span data-reader-contact="email" className="mt-0.5 block text-xs font-normal text-gray-500 dark:text-gray-400">
          <span className="sr-only">{t('profile.email')} </span>
          {contact.email}
        </span>
      ) : null}
      {contact.phone ? (
        <span data-reader-contact="phone" className="block text-xs font-normal text-gray-500 dark:text-gray-400">
          <span className="sr-only">{t('profile.phone')} </span>
          {contact.phone}
        </span>
      ) : null}
    </span>
  );
}
