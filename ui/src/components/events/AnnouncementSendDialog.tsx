import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { AlertCircle, Mail } from 'lucide-react';
import { Button, Modal } from '@/components/common';
import api from '@/services/api';
import type { Event } from '@/types';

export interface AnnouncementSendDialogProps {
  event: Event | null;
  isSending: boolean;
  onClose: () => void;
  onConfirm: (event: Event) => void;
}

export default function AnnouncementSendDialog({
  event,
  isSending,
  onClose,
  onConfirm,
}: AnnouncementSendDialogProps) {
  const { t, i18n } = useTranslation();
  const countQuery = useQuery({
    queryKey: ['event-announcement-recipient-count', event?.id],
    queryFn: () => api.getEventAnnouncementRecipientCount(event!.id),
    enabled: event != null,
    staleTime: 0,
  });

  const count = countQuery.data;
  const countMessage = countQuery.isLoading
    ? t('eventsConsent.recipientCountLoading')
    : countQuery.isError
      ? t('eventsConsent.recipientCountError')
      : count === 0
        ? t('eventsConsent.recipientCountZero')
        : t('eventsConsent.recipientCount', { count: count ?? 0 });

  return (
    <Modal
      isOpen={event != null}
      onClose={onClose}
      title={
        event?.announcementSentAt
          ? t('events.announcementAlreadySentTitle')
          : t('eventsConsent.confirmSendTitle')
      }
      size="sm"
      footer={
        <div className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isSending}>
            {t('common.cancel')}
          </Button>
          <Button
            type="button"
            onClick={() => {
              if (event) onConfirm(event);
            }}
            isLoading={isSending}
            disabled={countQuery.isLoading || countQuery.isFetching}
            leftIcon={<Mail className="h-4 w-4" />}
          >
            {event?.announcementSentAt ? t('events.sendAnywayBtn') : t('events.sendAnnouncement')}
          </Button>
        </div>
      }
    >
      <div className="space-y-3 text-sm text-gray-700 dark:text-gray-300">
        {event?.announcementSentAt ? (
          <div className="flex items-start gap-3">
            <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-amber-500" aria-hidden />
            <p>
              {t('events.announcementAlreadySentBody', {
                date: new Date(event.announcementSentAt).toLocaleString(i18n.language),
                name: event.name,
              })}
            </p>
          </div>
        ) : (
          <p>{t('eventsConsent.confirmSendBody')}</p>
        )}
        <p
          role="status"
          className={
            count === 0 && !countQuery.isLoading && !countQuery.isError
              ? 'rounded-lg border border-amber-300 bg-amber-50 px-3 py-2 font-medium text-amber-950 dark:border-amber-700 dark:bg-amber-950/40 dark:text-amber-100'
              : 'rounded-lg border border-gray-200 bg-gray-50 px-3 py-2 dark:border-gray-700 dark:bg-gray-900/60'
          }
        >
          {countMessage}
        </p>
      </div>
    </Modal>
  );
}
