/** Name plus optional sign-in or card fields used by patron pickers. */
export interface PatronNameSource {
  id?: string | null;
  firstname?: string | null;
  lastname?: string | null;
  login?: string | null;
  barcode?: string | null;
}

/** Display name for embedded user in lists (holds, loans, etc.). */
export function formatUserShortName(user: PatronNameSource | null | undefined): string {
  if (!user) return '';
  const name = [user.firstname, user.lastname].filter(Boolean).join(' ').trim();
  return name || user.id || '';
}

/**
 * Sign-in or card number shown beside a patron name.
 * The internal id is omitted: short numeric ids were glued onto the name.
 */
export function patronOptionSecondary(user: PatronNameSource | null | undefined): string | null {
  if (!user) return null;
  const login = user.login?.trim();
  if (login) return login;
  const barcode = user.barcode?.trim();
  if (barcode) return barcode;
  return null;
}
