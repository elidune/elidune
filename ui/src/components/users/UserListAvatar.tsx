import { useTranslation } from 'react-i18next';
import type { AccountTypeDefinition } from '@/types';
import { localizedAccountTypeLabel } from '@/utils/accountTypeDisplay';
import { accountTypeVisual } from '@/utils/userAccountTypeVisual';

type UserListAvatarProps = {
  firstname?: string | null;
  lastname?: string | null;
  accountType?: string | null;
  accountTypes?: readonly AccountTypeDefinition[];
  size?: 'md' | 'lg' | 'xl';
  className?: string;
};

const SIZE = {
  md: {
    circle: 'h-10 w-10 text-sm',
    overlay: 'h-[1.125rem] w-[1.125rem] -bottom-px -right-px',
    overlayIcon: 'h-3 w-3',
  },
  lg: {
    circle: 'h-12 w-12 text-sm',
    overlay: 'h-5 w-5 bottom-0 right-0',
    overlayIcon: 'h-3 w-3',
  },
  xl: {
    circle: 'h-16 w-16 text-xl font-bold',
    overlay: 'h-6 w-6 -bottom-0.5 -right-0.5',
    overlayIcon: 'h-3.5 w-3.5',
  },
} as const;

export default function UserListAvatar({
  firstname,
  lastname,
  accountType,
  accountTypes,
  size = 'md',
  className = '',
}: UserListAvatarProps) {
  const { t } = useTranslation();
  const visual = accountTypeVisual(accountType);
  const OverlayIcon = visual.overlayIcon;
  const dims = SIZE[size];
  const initials = `${firstname?.[0] || '?'}${lastname?.[0] || ''}`;
  const roleLabel = localizedAccountTypeLabel(t, accountType, accountTypes);

  return (
    <div
      className={['relative shrink-0', className].filter(Boolean).join(' ')}
      aria-label={roleLabel !== '—' ? roleLabel : undefined}
      title={roleLabel !== '—' ? roleLabel : undefined}
    >
      <div
        className={[
          'flex items-center justify-center rounded-full font-medium',
          dims.circle,
          visual.circleClass,
          visual.initialsClass,
        ].join(' ')}
      >
        <span>{initials}</span>
      </div>
      {OverlayIcon ? (
        <span
          className={[
            'pointer-events-none absolute flex items-center justify-center rounded-full',
            'bg-white ring-2 ring-white dark:bg-gray-900 dark:ring-gray-900',
            dims.overlay,
          ].join(' ')}
          aria-hidden
        >
          <OverlayIcon className={[dims.overlayIcon, visual.overlayIconClass].join(' ')} />
        </span>
      ) : null}
    </div>
  );
}
