import { useTranslation } from 'react-i18next';
import type { PublicType, UserShort } from '@/types';
import { localizedPublicTypeById } from '@/utils/publicTypeDisplay';

type UserListProfileBadgesProps = {
  user: UserShort;
  publicTypes: PublicType[];
  className?: string;
};

export default function UserListProfileBadges({
  user,
  publicTypes,
  className = '',
}: UserListProfileBadgesProps) {
  const { t } = useTranslation();
  const publicLabel = localizedPublicTypeById(t, publicTypes, user.publicType);

  if (!publicLabel) return null;

  return (
    <div className={className}>
      <span className="block pl-4 text-xs text-gray-600 dark:text-gray-400">{publicLabel}</span>
    </div>
  );
}
