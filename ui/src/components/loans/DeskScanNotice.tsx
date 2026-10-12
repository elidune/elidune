import { useTranslation } from 'react-i18next';
import { Link } from 'react-router-dom';
import { AlertTriangle } from 'lucide-react';
import Button from '@/components/common/Button';
import { patronId } from '@/utils/patronLink';
import {
  checkoutOnLoanCopy,
  deskScanAlertMessage,
  deskScanNoticeAction,
  type DeskScanAlert,
  type DeskScanPlace,
} from '@/utils/deskScanAlert';

interface DeskScanNoticeProps {
  alert: DeskScanAlert;
  /** Checkout asks for a return first. Return only reports a loan or a hold. */
  place?: DeskScanPlace;
  /**
   * Switches to the return screen with the barcode already filled in.
   * Offered only when the copy is on loan to another reader. It does not create a loan.
   */
  onReturn?: () => void;
  /**
   * Renews the loan already open for this reader.
   * Offered when checkout is refused because the copy is already on loan to them.
   */
  onRenew?: () => void;
  renewing?: boolean;
  /**
   * When true, the other borrower's card number or initials links to their record.
   * Without read permission the identifier stays plain text. The full name is never shown.
   */
  canReadBorrowers?: boolean;
}

/**
 * Inline notice for a scanned copy that is on loan to another reader or on hold.
 * `role="status"` keeps it out of the scan field’s focus so the next barcode can be entered.
 * There is no force-loan action.
 */
function BorrowerIdentifier({
  sentenceKey,
  identifier,
  userId,
  linked,
}: {
  sentenceKey: string;
  identifier: string;
  userId: string | null;
  linked: boolean;
}) {
  const { t } = useTranslation();
  const marker = '\u0000';
  const sentence = t(sentenceKey, { identifier: marker });
  const splitAt = sentence.indexOf(marker);
  const before = splitAt >= 0 ? sentence.slice(0, splitAt) : sentence;
  const after = splitAt >= 0 ? sentence.slice(splitAt + marker.length) : '';
  const id = patronId(userId);
  const value =
    linked && id ? (
      <Link
        to={`/users/${id}`}
        className="underline decoration-amber-700/70 underline-offset-2 hover:text-amber-800 dark:hover:text-amber-50"
      >
        {identifier}
      </Link>
    ) : (
      identifier
    );
  return (
    <>
      {before}
      {value}
      {after}
    </>
  );
}

export default function DeskScanNotice({
  alert,
  place = 'checkout',
  onReturn,
  onRenew,
  renewing = false,
  canReadBorrowers = false,
}: DeskScanNoticeProps) {
  const { t } = useTranslation();
  const action = deskScanNoticeAction(alert, place);
  const copy = place === 'checkout' && alert.kind === 'onLoan' ? checkoutOnLoanCopy(alert) : null;
  return (
    <div
      role="status"
      className="mt-3 flex flex-col gap-2 rounded-lg border border-amber-200 bg-amber-50 p-3 text-amber-950 dark:border-amber-900/60 dark:bg-amber-950/30 dark:text-amber-100 sm:flex-row sm:items-center"
    >
      <div className="flex min-w-0 flex-1 items-start gap-2">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />
        <p className="text-sm font-medium">
          {copy ? (
            <BorrowerIdentifier
              sentenceKey={copy.kind === 'card' ? 'loans.scanAlertOnLoanCard' : 'loans.scanAlertOnLoan'}
              identifier={copy.label}
              userId={copy.userId}
              linked={canReadBorrowers}
            />
          ) : (
            deskScanAlertMessage(alert, (key, values) => t(key, values), place)
          )}
        </p>
      </div>
      {action === 'return' && onReturn ? (
        <Button type="button" size="sm" variant="secondary" onClick={onReturn}>
          {t('loans.scanAlertGoToReturn')}
        </Button>
      ) : null}
      {action === 'renew' && onRenew ? (
        <Button type="button" size="sm" variant="secondary" onClick={onRenew} isLoading={renewing} disabled={renewing}>
          {t('loans.scanAlertRenew')}
        </Button>
      ) : null}
    </div>
  );
}
