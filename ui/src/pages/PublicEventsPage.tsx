import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { CalendarDays, Plus } from 'lucide-react';
import { Button, Card, PageHeading } from '@/components/common';
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
      <PageHeading
        icon={CalendarDays}
        title={t('events.title')}
        subtitle={t('home.upcomingEventsSubtitle')}
        actions={
          canManage ? (
            <Button
              type="button"
              leftIcon={<Plus className="h-4 w-4" />}
              onClick={() => setShowCreateModal(true)}
            >
              {t('events.newEvent')}
            </Button>
          ) : undefined
        }
      />

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
