import { useTranslation } from 'react-i18next';
import ReaderIdentifier from '@/components/users/ReaderIdentifier';
import PublicCopyReturnLine from '@/components/items/PublicCopyReturnLine';
import {
  formatCopyDueDate,
  isCopyBorrowed,
  isCopyOnHold,
  itemBorrowerIdentity,
  publicOnLoanLabel,
  type CopyCirculationFields,
  type PublicCopyStatusFields,
} from '@/utils/copyLoanStatus';

type CopyLoanStatusProps = {
  /** Staff link to the reader record. Ignored on the public view. */
  linked?: boolean;
} & (
  | { audience: 'staff'; copy: CopyCirculationFields }
  | { audience: 'public'; copy: PublicCopyStatusFields }
);

/**
 * Circulation lines on an item card.
 * Staff never load the loan: without a borrower on the item they show “Emprunté” only.
 * Readers and the OPAC take {@link PublicCopyStatusFields}, which has no `dueDate`.
 * An on-loan copy with no expected return date is “On loan” only.
 */
export default function CopyLoanStatus({ audience, copy, linked = false }: CopyLoanStatusProps) {
  const { t, i18n } = useTranslation();
  const borrowed = isCopyBorrowed(copy);
  const onHold = isCopyOnHold(copy);

  const assignedBesideStatus = copy.onHold === true && !onHold;

  if (audience !== 'staff') {
    const loanLabel = publicOnLoanLabel(t, copy, i18n.language);
    if (!loanLabel && !onHold && !assignedBesideStatus) return null;
    return (
      <div data-copy-status="public">
        {loanLabel ? (
          <PublicCopyReturnLine
            availability={copy.availability}
            expectedReturnDate={copy.expectedReturnDate}
            className="text-sm text-gray-500 dark:text-gray-400"
          />
        ) : null}
        {onHold ? <p className="text-sm text-gray-500 dark:text-gray-400">{t('availability.onHold')}</p> : null}
        {assignedBesideStatus ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('items.holdWaiting')}</p>
        ) : null}
      </div>
    );
  }

  if (!borrowed && !assignedBesideStatus) return null;
  const identity = borrowed ? itemBorrowerIdentity(copy.borrower) : null;
  const due = borrowed ? formatCopyDueDate(copy.dueDate, i18n.language) : null;
  return (
    <div data-copy-status="staff" className="text-sm text-gray-500 dark:text-gray-400">
      {borrowed ? (
        <p>
          {identity ? (
            <>
              {t('items.borrowedBy')} <ReaderIdentifier user={identity} linked={linked} />
            </>
          ) : (
            t('items.borrowed')
          )}
        </p>
      ) : null}
      {due ? <p>{t('items.dueBack', { date: due })}</p> : null}
      {assignedBesideStatus ? <p>{t('items.holdWaiting')}</p> : null}
    </div>
  );
}
