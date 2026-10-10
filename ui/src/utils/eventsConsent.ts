import type { TFunction } from 'i18next';
import type { EventsConsentSource } from '@/types';

const KNOWN_SOURCES: readonly EventsConsentSource[] = [
  'migration',
  'registration',
  'desk',
  'profile',
  'unsubscribe',
];

export function normalizeEventsConsentSource(
  value: string | null | undefined
): EventsConsentSource | null {
  if (!value) return null;
  const normalized = value.trim().toLowerCase();
  if (!(KNOWN_SOURCES as readonly string[]).includes(normalized)) return null;
  return normalized as EventsConsentSource;
}

function formatConsentDate(at: string | null | undefined, language: string): string {
  if (!at) return '';
  const parsed = new Date(at);
  if (Number.isNaN(parsed.getTime())) return '';
  return parsed.toLocaleDateString(language);
}

/**
 * Plain-language record of when and how consent was stored.
 * Returns null when the server has not recorded a source or a date.
 */
export function eventsConsentStatusText(
  t: TFunction,
  language: string,
  consent: boolean | null | undefined,
  at: string | null | undefined,
  source: string | null | undefined
): string | null {
  const normalized = normalizeEventsConsentSource(source);
  const optedIn = consent === true;
  const date = formatConsentDate(at, language);
  const withDate = (withDateKey: string, noDateKey: string) =>
    date ? t(withDateKey, { date }) : t(noDateKey);

  switch (normalized) {
    case 'migration':
      return date
        ? t('eventsConsent.status.migrationOn', { date })
        : t('eventsConsent.status.migration');
    case 'desk':
      return optedIn
        ? withDate('eventsConsent.status.deskGiven', 'eventsConsent.status.deskGivenNoDate')
        : withDate('eventsConsent.status.deskDeclined', 'eventsConsent.status.deskDeclinedNoDate');
    case 'registration':
      return optedIn
        ? withDate(
            'eventsConsent.status.registrationGiven',
            'eventsConsent.status.registrationGivenNoDate'
          )
        : withDate(
            'eventsConsent.status.registrationDeclined',
            'eventsConsent.status.registrationDeclinedNoDate'
          );
    case 'profile':
      return optedIn
        ? withDate('eventsConsent.status.profileGiven', 'eventsConsent.status.profileGivenNoDate')
        : withDate(
            'eventsConsent.status.profileDeclined',
            'eventsConsent.status.profileDeclinedNoDate'
          );
    case 'unsubscribe':
      return withDate(
        'eventsConsent.status.unsubscribe',
        'eventsConsent.status.unsubscribeNoDate'
      );
    default:
      if (!date) return null;
      return optedIn
        ? t('eventsConsent.status.givenOn', { date })
        : t('eventsConsent.status.declinedOn', { date });
  }
}
