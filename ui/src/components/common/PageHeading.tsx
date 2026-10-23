import { type ComponentType, type ReactNode } from 'react';

export type PageHeadingProps = {
  title: ReactNode;
  subtitle: ReactNode;
  icon?: ComponentType<{ className?: string }>;
  iconClassName?: string;
  iconTileClassName?: string;
  actions?: ReactNode;
  className?: string;
  titleClassName?: string;
  subtitleClassName?: string;
};

const defaultSubtitleClass = 'mt-1 text-sm text-gray-500 dark:text-gray-400';
const defaultIconTileClass = 'bg-amber-100 dark:bg-amber-900/40';
const defaultIconClass = 'text-amber-700 dark:text-amber-300';

export default function PageHeading({
  title,
  subtitle,
  icon: Icon,
  iconClassName = defaultIconClass,
  iconTileClassName = defaultIconTileClass,
  actions,
  className = '',
  titleClassName = '',
  subtitleClassName = defaultSubtitleClass,
}: PageHeadingProps) {
  return (
    <div
      className={[
        'flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      <div className="flex min-w-0 flex-1 items-stretch gap-3">
        {Icon ? (
          <div
            className={[
              'flex w-14 shrink-0 items-center justify-center rounded-xl',
              iconTileClassName,
            ]
              .filter(Boolean)
              .join(' ')}
          >
            <Icon className={['h-8 w-8', iconClassName].join(' ')} aria-hidden />
          </div>
        ) : null}
        <div className="min-w-0">
          <h1
            className={[
              'text-2xl font-bold text-gray-900 dark:text-white',
              titleClassName,
            ]
              .filter(Boolean)
              .join(' ')}
          >
            {title}
          </h1>
          {subtitle ? <div className={subtitleClassName}>{subtitle}</div> : null}
        </div>
      </div>
      {actions ? (
        <div className="flex w-full shrink-0 flex-wrap gap-2 sm:w-auto sm:justify-end">{actions}</div>
      ) : null}
    </div>
  );
}
