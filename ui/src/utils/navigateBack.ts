import type { NavigateFunction, To } from 'react-router-dom';

/**
 * Browser-style back for detail-page headers. Uses session history when possible;
 * otherwise navigates to `fallback` (e.g. direct URL open or new tab).
 */
export function navigateBack(navigate: NavigateFunction, fallback: To) {
  const idx = window.history.state?.idx;
  if (typeof idx === 'number' && idx > 0) {
    navigate(-1);
    return;
  }
  navigate(fallback);
}
