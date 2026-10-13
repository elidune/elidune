import { useTranslation } from 'react-i18next';
import Button from '@/components/common/Button';
import { circulationMayForce, type CirculationForceSurface } from '@/utils/circulationForce';

interface CirculationForceButtonProps {
  surface: CirculationForceSurface;
  code: string | null | undefined;
  onForce: () => void;
  disabled?: boolean;
}

/**
 * Staff retry for a forceable circulation refusal.
 * `card_lost` renders nothing: the refusal cannot be overridden.
 */
export default function CirculationForceButton({
  surface,
  code,
  onForce,
  disabled = false,
}: CirculationForceButtonProps) {
  const { t } = useTranslation();
  if (!circulationMayForce(surface, code)) return null;
  return (
    <Button type="button" size="sm" variant="secondary" onClick={onForce} disabled={disabled}>
      {t('loans.forceLoan')}
    </Button>
  );
}
