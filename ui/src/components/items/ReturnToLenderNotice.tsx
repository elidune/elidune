import { AlertTriangle } from 'lucide-react';

interface ReturnToLenderNoticeProps {
  message: string;
}

/**
 * Check-in of a deposit copy whose batch is returning.
 * The copy goes back to the lender and is not shelved.
 */
export default function ReturnToLenderNotice({ message }: ReturnToLenderNoticeProps) {
  return (
    <div
      role="alert"
      data-return-to-lender
      className="flex items-start gap-3 rounded-lg border-2 border-amber-500 bg-amber-50 p-4 text-amber-950 dark:border-amber-400 dark:bg-amber-950/50 dark:text-amber-50"
    >
      <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-amber-700 dark:text-amber-300" aria-hidden />
      <p className="text-base font-semibold leading-snug">{message}</p>
    </div>
  );
}
