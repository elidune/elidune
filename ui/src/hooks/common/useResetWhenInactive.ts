import { useState } from 'react';

/** Reset local state during render when `active` becomes false (no effect). */
export function useResetWhenInactive(active: boolean, reset: () => void) {
  const [wasActive, setWasActive] = useState(active);
  if (active !== wasActive) {
    setWasActive(active);
    if (!active) reset();
  }
}
