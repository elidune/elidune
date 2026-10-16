/** Auto-fit grid: each field column caps around 16rem instead of stretching with the page. */
export const FORM_FIELD_AUTO_GRID =
  'grid items-start gap-x-4 gap-y-4 [grid-template-columns:repeat(auto-fill,minmax(min(100%,12.5rem),16rem))]';

/** Slightly wider cells (e-mail, street block). */
export const FORM_FIELD_AUTO_GRID_WIDE =
  'grid items-start gap-x-4 gap-y-4 [grid-template-columns:repeat(auto-fill,minmax(min(100%,14rem),20rem))]';

/** One field per row; pair rows only where it helps scanning (names, postal code + city). */
export const FORM_FIELD_STACK = 'flex flex-col gap-4';

/** Two related fields on sm+; single column on narrow viewports. */
export const FORM_FIELD_PAIR_ROW =
  'grid w-full grid-cols-1 items-start gap-4 sm:grid-cols-2 sm:max-w-xl';

/** Width caps for single fields outside a grid (still full width on very narrow viewports). */
export const FORM_FIELD_WIDTH = {
  zip: 'w-full max-w-[9.5rem]',
  short: 'w-full max-w-[11.5rem]',
  medium: 'w-full max-w-xs',
  long: 'w-full max-w-sm',
  email: 'w-full max-w-md',
  street: 'w-full max-w-xl',
  password: 'w-full max-w-sm',
} as const;
