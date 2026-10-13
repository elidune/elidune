import type { PublicType } from '@/types';
import { localizedPublicTypeLabel } from '@/utils/publicTypeDisplay';

export interface EventAudienceSource {
  allAudiences?: boolean;
  publicTypes?: string[] | null;
  /** Legacy single audience, kept so older payloads still render. */
  publicType?: string | null;
}

/** Label for the event's audiences. `allLabel` is the explicit "all audiences" caption. */
export function eventAudienceDisplayLabel(
  event: EventAudienceSource,
  publicTypes: PublicType[],
  allLabel: string,
  t: (key: string) => string,
): string {
  if (event.allAudiences) return allLabel;
  const names = (event.publicTypes ?? []).map((name) => name.trim()).filter((name) => name !== '');
  const legacy = event.publicType?.trim() ?? '';
  if (names.length === 0 && legacy !== '') {
    names.push(legacy);
  }
  if (names.length === 0) return '—';
  return names
    .map((name) => {
      const match = publicTypes.find((pt) => pt.name === name);
      return match ? localizedPublicTypeLabel(t, match) : localizedPublicTypeLabel(t, { name, label: name });
    })
    .join(', ');
}
