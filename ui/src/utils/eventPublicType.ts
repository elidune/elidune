import type { TFunction } from 'i18next';
import type { PublicType } from '@/types';
import { getApiErrorMessage, getApiErrorRawMessage, getHttpStatus } from '@/utils/apiError';
import { localizedPublicTypeLabel } from '@/utils/publicTypeDisplay';

export interface EventAudienceSource {
  allAudiences?: boolean;
  publicTypes?: string[] | null;
}

export interface EventAudienceSelection {
  allAudiences: boolean;
  publicTypes: readonly string[];
}

/** Multi-select option value for « all audiences » (not a catalog public type name). */
export const EVENT_ALL_AUDIENCES_VALUE = '__all_audiences__';

export function eventAudienceMultiSelectValue(selection: EventAudienceSelection): string[] {
  if (selection.allAudiences) return [EVENT_ALL_AUDIENCES_VALUE];
  return [...selection.publicTypes];
}

export function eventAudienceFromMultiSelect(
  next: readonly string[],
  current: EventAudienceSelection,
): EventAudienceSelection {
  if (next.includes(EVENT_ALL_AUDIENCES_VALUE)) {
    return applyAllAudiencesToggle(current, true);
  }
  return { allAudiences: false, publicTypes: normalizeAudienceNames(next) };
}

/** Fields written on create and update. Legacy `publicType` is never included. */
export interface EventAudienceWrite {
  allAudiences: boolean;
  publicTypes: string[];
}

export type EventAudienceRejectReason = 'empty' | 'combined';

export type EventAudienceResolution =
  | { ok: true; payload: EventAudienceWrite }
  | { ok: false; reason: EventAudienceRejectReason };

/** Trim, drop blanks, and keep the first occurrence of each name. */
export function normalizeAudienceNames(names: readonly string[]): string[] {
  const out: string[] = [];
  for (const raw of names) {
    const name = raw.trim();
    if (name === '' || out.includes(name)) continue;
    out.push(name);
  }
  return out;
}

/** Turning All audiences on clears the list. Turning it off leaves the list as it is. */
export function applyAllAudiencesToggle(
  current: EventAudienceSelection,
  allAudiences: boolean,
): EventAudienceSelection {
  if (allAudiences) return { allAudiences: true, publicTypes: [] };
  return { allAudiences: false, publicTypes: normalizeAudienceNames(current.publicTypes) };
}

/**
 * Client-side audience check, matching the server:
 * at least one choice, and `allAudiences` never together with a non-empty list.
 */
export function resolveEventAudiences(selection: EventAudienceSelection): EventAudienceResolution {
  const publicTypes = normalizeAudienceNames(selection.publicTypes);
  if (selection.allAudiences && publicTypes.length > 0) {
    return { ok: false, reason: 'combined' };
  }
  if (!selection.allAudiences && publicTypes.length === 0) {
    return { ok: false, reason: 'empty' };
  }
  return {
    ok: true,
    payload: {
      allAudiences: selection.allAudiences,
      publicTypes: selection.allAudiences ? [] : publicTypes,
    },
  };
}

export function eventAudienceErrorKey(reason: EventAudienceRejectReason): string {
  return reason === 'combined' ? 'events.targetPublicCombined' : 'events.targetPublicRequired';
}

/**
 * Text for a failed event save. A 400 body message is shown as the server sent it.
 * `validation_error` would otherwise hide that text behind a generic translation.
 */
export function eventSaveErrorMessage(error: unknown, t: TFunction): string {
  if (getHttpStatus(error) === 400) {
    const raw = getApiErrorRawMessage(error);
    if (raw) return raw;
  }
  return getApiErrorMessage(error, t);
}

/** Label for the event's audiences. `allLabel` is the explicit "all audiences" caption. */
export function eventAudienceDisplayLabel(
  event: EventAudienceSource,
  publicTypes: PublicType[],
  allLabel: string,
  t: (key: string) => string,
): string {
  if (event.allAudiences) return allLabel;
  const names = normalizeAudienceNames(event.publicTypes ?? []);
  if (names.length === 0) return '—';
  return names
    .map((name) => {
      const match = publicTypes.find((pt) => pt.name === name);
      return match ? localizedPublicTypeLabel(t, match) : localizedPublicTypeLabel(t, { name, label: name });
    })
    .join(', ');
}
