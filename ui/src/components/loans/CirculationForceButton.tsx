import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import { circulationMayForce, type CirculationForceSurface } from '@/utils/circulationForce';

interface CirculationForceButtonProps {
  surface: CirculationForceSurface;
  code: string | null | undefined;
  /** Loans write. Without it the refusal stays a message. */
  canForce: boolean;
  onForce: () => void;
  disabled?: boolean;
}

/**
 * Staff retry for a forceable circulation refusal.
 * Hidden unless the code is in the force map for this surface and `canForce` is true.
 */
export default function CirculationForceButton({
  surface,
  code,
  canForce,
  onForce,
  disabled = false,
}: CirculationForceButtonProps) {
  const { t } = useTranslation();
  if (!circulationMayForce(surface, code, canForce)) return null;
  return (
    <Button type="button" size="sm" variant="secondary" onClick={onForce} disabled={disabled}>
      {t('loans.forceLoan')}
    </Button>
  );
}
