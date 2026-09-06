import { AlertTriangle, HelpCircle, PackageX, SearchCheck, SearchX } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Badge, Button } from '@/components/common';
import type { CirculationExceptionKind } from '@/hooks/loans/useCirculationExceptionAction';

interface LoanExceptionActionsProps {
  loanId: string;
  loanLabel?: string;
  claimedReturned?: boolean;
  disabled?: boolean;
  busyKind?: CirculationExceptionKind | null;
  onOpen: (kind: CirculationExceptionKind, label?: string) => void;
}

export default function LoanExceptionActions({
  loanLabel,
  claimedReturned = false,
  disabled = false,
  busyKind = null,
  onOpen,
}: LoanExceptionActionsProps) {
  const { t } = useTranslation();

  const action = (kind: CirculationExceptionKind) => {
    onOpen(kind, loanLabel);
  };

  if (claimedReturned) {
    return (
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="warning" size="sm">{t('loans.exceptions.claimedBadge')}</Badge>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          leftIcon={<SearchCheck className="h-4 w-4" />}
          isLoading={busyKind === 'resolveFound'}
          disabled={disabled}
          onClick={(e) => {
            e.stopPropagation();
            action('resolveFound');
          }}
        >
          {t('loans.exceptions.resolveFound')}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          leftIcon={<SearchX className="h-4 w-4" />}
          isLoading={busyKind === 'resolveNotFound'}
          disabled={disabled}
          onClick={(e) => {
            e.stopPropagation();
            action('resolveNotFound');
          }}
        >
          {t('loans.exceptions.resolveNotFound')}
        </Button>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          leftIcon={<PackageX className="h-4 w-4" />}
          isLoading={busyKind === 'lost'}
          disabled={disabled}
          onClick={(e) => {
            e.stopPropagation();
            action('lost');
          }}
        >
          {t('loans.exceptions.lost')}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap items-center gap-2">
      <Button
        type="button"
        size="sm"
        variant="ghost"
        leftIcon={<PackageX className="h-4 w-4" />}
        isLoading={busyKind === 'lost'}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          action('lost');
        }}
      >
        {t('loans.exceptions.lost')}
      </Button>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        leftIcon={<AlertTriangle className="h-4 w-4" />}
        isLoading={busyKind === 'damaged'}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          action('damaged');
        }}
      >
        {t('loans.exceptions.damaged')}
      </Button>
      <Button
        type="button"
        size="sm"
        variant="ghost"
        leftIcon={<HelpCircle className="h-4 w-4" />}
        isLoading={busyKind === 'claimedReturned'}
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          action('claimedReturned');
        }}
      >
        {t('loans.exceptions.claimedReturned')}
      </Button>
    </div>
  );
}
