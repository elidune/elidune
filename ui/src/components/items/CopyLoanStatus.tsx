import { useTranslation } from 'react-i18next';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import {
  formatCopyDueDate,
  isCopyBorrowed,
  isCopyOnHold,
  itemBorrowerIdentity,
  type CopyCirculationFields,
} from '@/utils/copyLoanStatus';

interface CopyLoanStatusProps {
  audience: 'staff' | 'public';
  copy: CopyCirculationFields;
  /** Staff link to the reader record. Ignored on the public view. */
  linked?: boolean;
}

/**
 * Circulation lines on an item card.
 * Staff never load the loan: without a borrower on the item they show “Emprunté” only.
 * Readers and the OPAC never receive a borrower identifier.
 */
export default function CopyLoanStatus({ audience, copy, linked = false }: CopyLoanStatusProps) {
  const { t, i18n } = useTranslation();
  const borrowed = isCopyBorrowed(copy);
  const onHold = isCopyOnHold(copy);

  if (audience !== 'staff') {
    const due = borrowed ? formatCopyDueDate(copy.dueDate, i18n.language) : null;
    if (!borrowed && !onHold) return null;
    return (
      <div data-copy-status="public">
        {borrowed ? <p className="text-sm text-gray-500 dark:text-gray-400">{t('items.borrowed')}</p> : null}
        {due ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('items.dueBack', { date: due })}</p>
        ) : null}
        {onHold ? <p className="text-sm text-gray-500 dark:text-gray-400">{t('availability.onHold')}</p> : null}
      </div>
    );
  }

  if (!borrowed) return null;
  const identity = itemBorrowerIdentity(copy.borrower);
  return (
    <p data-copy-status="staff" className="text-sm text-gray-500 dark:text-gray-400">
      {identity ? (
        <>
          {t('items.borrowedBy')} <ReaderIdentifier user={identity} linked={linked} />
        </>
      ) : (
        t('items.borrowed')
      )}
    </p>
  );
}
