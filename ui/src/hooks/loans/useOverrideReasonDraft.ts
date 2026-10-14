import { useEffect, useRef, useState } from 'react';

/**
 * Reason textarea state for a force dialog.
 * The draft resets when `resetKey` changes (a new refusal), and stays put
 * when only the error text changes, so a 422 can be shown without wiping the field.
 */
export function useOverrideReasonDraft(
  resetKey: string,
  focusWhen: boolean,
  focusToken: string | null | undefined,
) {
  const [draft, setDraft] = useState({ key: resetKey, reason: '' });
  if (draft.key !== resetKey) {
    setDraft({ key: resetKey, reason: '' });
  }
  const reason = draft.key === resetKey ? draft.reason : '';
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!focusWhen) return;
    const focus = () => inputRef.current?.focus();
    const frame = requestAnimationFrame(focus);
    const timer = window.setTimeout(focus, 30);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timer);
    };
  }, [focusWhen, resetKey, focusToken]);

  return {
    reason,
    setReason: (value: string) => setDraft({ key: resetKey, reason: value }),
    inputRef,
  };
}
