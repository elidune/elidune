import type { UpdateProfileRequest } from '@/types';

/** Fields the signed-in user may send on `PUT /auth/profile`. Birth date is never included. */
export interface SelfProfileForm {
  firstname: string;
  lastname: string;
  email: string;
  phone: string;
  login: string;
  addrStreet: string;
  addrZipCode: string;
  addrCity: string;
  eventsConsent: boolean;
}

/**
 * Body for `PUT /auth/profile`.
 * A different birth date is `403`. The client omits the field so the stored date stays.
 * Staff with users write change a birth date on `PUT /users/{id}`.
 */
export function selfProfilePayload(
  form: SelfProfileForm,
  includeEventsConsent: boolean,
): UpdateProfileRequest {
  const zip = form.addrZipCode.trim();
  const parsedZip = zip ? Number.parseInt(zip, 10) : Number.NaN;
  const body: UpdateProfileRequest = {
    firstname: form.firstname || undefined,
    lastname: form.lastname || undefined,
    email: form.email || undefined,
    phone: form.phone || undefined,
    login: form.login || undefined,
    addrStreet: form.addrStreet || undefined,
    addrZipCode: Number.isInteger(parsedZip) ? parsedZip : undefined,
    addrCity: form.addrCity || undefined,
  };
  if (includeEventsConsent) body.eventsConsent = form.eventsConsent;
  return body;
}
