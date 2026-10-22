import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
import { Card, Button } from '@/components/common';
import PublicEventsPanel from '@/components/events/PublicEventsPanel';
import EventManageDialogs from '@/components/events/EventManageDialogs';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import { staffAccess } from '@/utils/staffAccess';
import type { Event } from '@/types';

export default function PublicEventsPage() {
  const { t } = useTranslation();
  const { user } = useAuth();

  const canManage = staffAccess(user, api.getToken()).eventsWrite;

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingEvent, setEditingEvent] = useState<Event | null>(null);
  const [eventPendingDelete, setEventPendingDelete] = useState<Event | null>(null);

  return (
    <div className="flex min-h-0 w-full min-w-0 flex-1 flex-col gap-4">
      <div className="flex shrink-0 flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">{t('home.upcomingEvents')}</h1>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('home.upcomingEventsSubtitle')}</p>
        </div>
        {canManage && (
          <Button
            type="button"
            className="shrink-0"
            leftIcon={<Plus className="h-4 w-4" />}
            onClick={() => setShowCreateModal(true)}
          >
            {t('events.newEvent')}
          </Button>
        )}
      </div>

      <Card
        padding="none"
        className="flex min-h-0 min-w-0 w-full flex-1 flex-col overflow-hidden"
      >
        <div className="flex min-h-0 min-w-0 w-full flex-1 overflow-hidden">
          <PublicEventsPanel
            queryKey={['app-public-events']}
            emptyMessage={t('opac.eventsEmpty')}
            onEdit={canManage ? setEditingEvent : undefined}
            onDelete={canManage ? setEventPendingDelete : undefined}
          />
        </div>
      </Card>

      {canManage && (
        <EventManageDialogs
          showCreate={showCreateModal}
          onCloseCreate={() => setShowCreateModal(false)}
          editingEvent={editingEvent}
          onCloseEdit={() => setEditingEvent(null)}
          eventPendingDelete={eventPendingDelete}
          onCloseDeleteConfirm={() => setEventPendingDelete(null)}
          onRequestDelete={(event) => {
            setEventPendingDelete(event);
            setEditingEvent(null);
          }}
        />
      )}
    </div>
  );
}
