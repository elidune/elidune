/** Fields `PUT /users/{own id}` accepts when `usersRights` is own (#217). */
export const OWN_CONTACT_KEYS = [
  'email',
  'phone',
  'addrStreet',
  'addrZipCode',
  'addrCity',
  'language',
  'eventsConsent',
  'receiveReminders',
] as const;

export type OwnContactKey = (typeof OWN_CONTACT_KEYS)[number];

export interface OwnContactForm {
  email: string;
  phone: string;
  addrStreet: string;
  addrZipCode: string;
  addrCity: string;
  eventsConsent: boolean;
  receiveReminders: boolean;
}

export interface OwnContactUpdate {
  email: string | null;
  phone: string | null;
  addrStreet: string | null;
  addrZipCode: number | null;
  addrCity: string | null;
  eventsConsent?: boolean;
  receiveReminders?: boolean;
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

/**
 * Body for a reader's own record. Identity, login, card number, status, and
 * fines are omitted so the server does not reject the whole update.
 */
export function ownContactPayload(
  form: OwnContactForm,
  options: { includeEventsConsent: boolean; includeReminders: boolean },
): OwnContactUpdate {
  const zip = form.addrZipCode.trim();
  const parsedZip = zip ? Number.parseInt(zip, 10) : Number.NaN;
  const body: OwnContactUpdate = {
    email: blankToNull(form.email),
    phone: blankToNull(form.phone),
    addrStreet: blankToNull(form.addrStreet),
    addrCity: blankToNull(form.addrCity),
    addrZipCode: Number.isInteger(parsedZip) ? parsedZip : null,
  };
  if (options.includeEventsConsent) body.eventsConsent = form.eventsConsent;
  if (options.includeReminders) body.receiveReminders = form.receiveReminders;
  return body;
}
