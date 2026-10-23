import type { ReactNode } from 'react';

type ListResultCountBarProps = {
  children: ReactNode;
  className?: string;
};

/** Total / result count row above a list table — right-aligned, shared across list pages. */
export default function ListResultCountBar({ children, className = '' }: ListResultCountBarProps) {
  return (
    <div
      className={[
        'flex shrink-0 justify-end border-b border-gray-200 px-4 py-2 text-right text-sm text-gray-600 dark:border-gray-800 dark:text-gray-300',
        className,
      ]
        .filter(Boolean)
        .join(' ')}
    >
      {children}
    </div>
  );
}
