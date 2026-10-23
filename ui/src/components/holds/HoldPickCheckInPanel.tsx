import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BookOpen, Check } from 'lucide-react';
import { BarcodeScanField } from '@/components/common';
import {
  pickCheckInBannerMessage,
  pickCheckInErrorMessage,
  runPickCheckInFlow,
  type PickCheckInBind,
} from '@/utils/pickCheckInFlow';
import type { PickCheckInNotice } from '@/utils/toPickDesk';

export interface HoldPickCheckInPanelProps {
  /** When set, POST includes `{ holdId, itemId }` so the scan must match this hold. */
  bind?: PickCheckInBind | null;
  onSuccess?: () => void;
  /** Compact layout for modals. */
  variant?: 'prominent' | 'compact';
  autoFocus?: boolean;
}

export default function HoldPickCheckInPanel({
  bind,
  onSuccess,
  variant = 'prominent',
  autoFocus = true,
}: HoldPickCheckInPanelProps) {
  const { t } = useTranslation();
  const inputRef = useRef<HTMLInputElement>(null);
  const [barcode, setBarcode] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastNotice, setLastNotice] = useState<PickCheckInNotice | null>(null);

  const submit = async (value: string) => {
    const trimmed = value.trim();
    if (!trimmed || busy) return;
    setBusy(true);
    setError(null);
    setLastNotice(null);
    try {
      const outcome = await runPickCheckInFlow(trimmed, bind, t);
      if (outcome.kind === 'pick') {
        setLastNotice(outcome.notice);
        setBarcode('');
        onSuccess?.();
      } else {
        setError(outcome.message);
      }
    } catch (e: unknown) {
      setError(pickCheckInErrorMessage(e, t));
    } finally {
      setBusy(false);
      if (autoFocus) {
        requestAnimationFrame(() => inputRef.current?.focus());
      }
    }
  };

  const ready = lastNotice?.kind === 'ready';
  const shell =
    variant === 'prominent'
      ? 'rounded-xl border border-emerald-200/80 bg-emerald-50/40 p-4 sm:p-5 dark:border-emerald-900/40 dark:bg-emerald-950/20'
      : 'space-y-3';

  return (
    <div className={shell}>
      <div className="space-y-1">
        <h3 className="text-base font-semibold text-gray-900 dark:text-white flex items-center gap-2">
          <Check className="h-5 w-5 text-emerald-600 dark:text-emerald-400 shrink-0" />
          {t('holds.checkInSectionTitle')}
        </h3>
        <p className="text-sm text-gray-600 dark:text-gray-400 leading-relaxed">
          {bind
            ? t('holds.checkInBoundHint', {
                title: bind.title?.trim() || t('pickList.noTitle'),
                barcode: bind.barcode?.trim() || '—',
              })
            : t('holds.checkInOpenHint')}
        </p>
      </div>

      <form
        className="mt-4 space-y-3 max-w-2xl"
        onSubmit={(e) => {
          e.preventDefault();
          void submit(barcode);
        }}
      >
        <BarcodeScanField
          ref={inputRef}
          value={barcode}
          onChange={(e) => {
            setBarcode(e.target.value);
            setError(null);
            setLastNotice(null);
          }}
          placeholder={t('loans.scanOrEnterBarcode')}
          leftIcon={<BookOpen className="h-4 w-4" />}
          autoFocus={autoFocus}
          disabled={busy}
          aria-busy={busy}
          scannerTitle={t('holds.checkInSectionTitle')}
          onCameraScan={(code) => {
            setBarcode(code);
            setError(null);
            setLastNotice(null);
            void submit(code);
          }}
        />
      </form>

      {error ? (
        <div
          role="alert"
          className="mt-3 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-800 dark:border-red-900/50 dark:bg-red-950/30 dark:text-red-200"
        >
          {error}
        </div>
      ) : null}

      {lastNotice ? (
        <div
          role="status"
          className={`mt-3 rounded-xl border px-4 py-3 text-sm font-semibold ${
            ready
              ? 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-900/50 dark:bg-emerald-950/30 dark:text-emerald-100'
              : 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900/50 dark:bg-amber-950/30 dark:text-amber-100'
          }`}
        >
          {pickCheckInBannerMessage(lastNotice, t)}
        </div>
      ) : null}
    </div>
  );
}
