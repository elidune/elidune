import { useTranslation } from 'react-i18next';
import { RotateCcw } from 'lucide-react';
import Button from '@/components/common/Button';
import type { MyLoanRenewalRefusal } from '@/types';
import { isAccountRenewalRefusal, renewalRefusalText } from '@/utils/apiError';

interface MyLoanRenewActionsProps {
  canRenew: boolean;
  renewalRefusalCode: MyLoanRenewalRefusal | null;
  renewalOpensAt: string | null;
  onRenew: () => void;
}

/** Patron renew control on « Mes prêts ». Account refusals are explained once in the page banner. */
export default function MyLoanRenewActions({
  canRenew,
  renewalRefusalCode,
  renewalOpensAt,
  onRenew,
}: MyLoanRenewActionsProps) {
  const { t, i18n } = useTranslation();
  const rowCode = isAccountRenewalRefusal(renewalRefusalCode) ? null : renewalRefusalCode;
  const refusal = renewalRefusalText(t, rowCode, renewalOpensAt, i18n.language);
  return (
    <>
      <Button
        size="sm"
        variant="secondary"
        disabled={!canRenew}
        onClick={onRenew}
        leftIcon={<RotateCcw className="h-4 w-4" />}
      >
        {t('loans.renew')}
      </Button>
      {refusal ? (
        <p className="text-sm text-gray-600 dark:text-gray-300 sm:text-right max-w-xs">{refusal}</p>
      ) : null}
    </>
  );
}
