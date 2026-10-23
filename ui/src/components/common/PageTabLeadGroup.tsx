import type { ReactNode } from 'react';

/** Tight title → tabs rhythm on mobile (title lives in the app header). */
export default function PageTabLeadGroup({
  children,
  className = '',
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={['flex min-w-0 flex-col gap-6 max-lg:gap-0', className].filter(Boolean).join(' ')}
    >
      {children}
    </div>
  );
}
