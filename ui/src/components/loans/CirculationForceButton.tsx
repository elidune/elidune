import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';

interface CirculationForceButtonProps {
  /** Server `forceable` on a loan refusal, or the renewal allowlist result. */
  forceable: boolean;
  /** Loans write. Without it the refusal stays a message. */
  canForce: boolean;
  onForce: () => void;
  disabled?: boolean;
}

/**
 * Staff retry for a forceable circulation refusal.
 * Hidden unless `forceable` is true and `canForce` is true.
 */
export default function CirculationForceButton({
  forceable,
  canForce,
  onForce,
  disabled = false,
}: CirculationForceButtonProps) {
  const { t } = useTranslation();
  if (!canForce || forceable !== true) return null;
  return (
    <Button type="button" size="sm" variant="secondary" onClick={onForce} disabled={disabled}>
      {t('loans.forceLoan')}
    </Button>
  );
}
