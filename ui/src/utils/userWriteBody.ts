import type { UserFormData } from '@/components/users/UserEditorForm';
import { appendReaderLanguage } from '@/utils/readerLanguage';
import { isReaderAccount } from '@/utils/readerPublicType';
import {
  dateInputToIsoEndOfDayUtc,
  defaultExpiryDateInputOneYearFromNow,
} from '@/utils/userSubscription';

/** Body for POST /users and PUT /users/{id}. A reader omits `publicType`; the server computes it. */
export function buildUserWriteBody(formData: UserFormData): Record<string, unknown> {
  const body: Record<string, unknown> = {
    login: formData.login || undefined,
    firstname: formData.firstname || undefined,
    lastname: formData.lastname || undefined,
    sex: formData.sex || undefined,
    email: formData.email || undefined,
    phone: formData.phone || undefined,
    barcode: formData.barcode || undefined,
    birthdate: formData.birthdate || undefined,
    addrStreet: formData.addrStreet || undefined,
    addrZipCode: formData.addrZipCode ? parseInt(formData.addrZipCode, 10) : undefined,
    addrCity: formData.addrCity || undefined,
    notes: formData.notes || undefined,
    fee: formData.fee || undefined,
    accountType: formData.accountType || undefined,
    ...(formData.expiryUnlimited
      ? { expiryAt: null }
      : {
          expiryAt: dateInputToIsoEndOfDayUtc(
            formData.expiryAt || defaultExpiryDateInputOneYearFromNow(),
          ),
        }),
  };
  if (!isReaderAccount(formData.accountType)) {
    body.publicType = formData.publicType ? String(formData.publicType) : undefined;
  }
  return appendReaderLanguage(body, formData.language);
}
