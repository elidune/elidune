export type LegalModalKind = 'about' | 'privacy';

export type LegalModalLocationState = {
  legalModal?: LegalModalKind;
};

export function readLegalModal(state: unknown): LegalModalKind | undefined {
  if (!state || typeof state !== 'object') return undefined;
  const kind = (state as LegalModalLocationState).legalModal;
  return kind === 'about' || kind === 'privacy' ? kind : undefined;
}
