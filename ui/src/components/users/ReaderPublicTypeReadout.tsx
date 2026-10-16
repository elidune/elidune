import { useTranslation } from 'react-i18next';
import type { PublicType } from '@/types';
import { formLabelClass } from '@/utils/formControl';
import { publicTypeCaption } from '@/utils/readerPublicType';

interface ReaderPublicTypeReadoutProps {
  id: string;
  publicTypes: readonly PublicType[];
  /** Staff records send the id. `GET /auth/me` sends the name. */
  publicType?: string | number | null;
  birthDateMissing?: boolean;
}

/** Read-only public type. Readers do not pick one; the server derives it from the birth date. */
export default function ReaderPublicTypeReadout({
  id,
  publicTypes,
  publicType,
  birthDateMissing = false,
}: ReaderPublicTypeReadoutProps) {
  const { t } = useTranslation();
  const caption = publicTypeCaption(t, publicTypes, publicType);

  return (
    <div id={id}>
      <p className={formLabelClass()}>{t('users.publicType')}</p>
      {caption ? (
        <p className="text-sm font-medium text-gray-900 dark:text-white">{caption}</p>
      ) : (
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('users.publicTypeFromBirthDate')}</p>
      )}
      {birthDateMissing ? (
        <p className="mt-1 text-sm text-amber-800 dark:text-amber-200">{t('users.birthDateMissing')}</p>
      ) : null}
    </div>
  );
}
