import { useState } from 'react';
import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useManageEventsQuery } from '@/hooks/events/useManageEventsQuery';
import {
  Plus,
  Calendar,
  CalendarDays,
  Clock,
  Users,
  Tag,
  ChevronLeft,
  History,
  Mail,
  CheckCircle,
  AlertCircle,
} from 'lucide-react';
import { Card, Button, Table, Pagination, ScrollableListRegion, ResponsiveRecordList, PageHeading, ListResultCountBar } from '@/components/common';
import AnnouncementSendDialog from '@/components/events/AnnouncementSendDialog';
import EventAttachmentLead from '@/components/events/EventAttachmentLead';
import EventManageDialogs from '@/components/events/EventManageDialogs';
import { EVENT_TYPES } from '@/components/events/eventDisplayConstants';
import api from '@/services/api';
import type { Event } from '@/types';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import { eventAudienceDisplayLabel } from '@/utils/eventPublicType';

const EVENTS_PER_PAGE = 20;

const EVENT_TYPE_COLORS: Record<number, string> = {
  0: 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300',
  1: 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300',
  2: 'bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300',
  3: 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300',
  4: 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300',
  5: 'bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300',
  6: 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300',
};

export default function EventsPage() {
  const { t, i18n } = useTranslation();
  const { data: publicTypes = [] } = usePublicTypesQuery();

  const [currentPage, setCurrentPage] = useState(1);
  const [showPast, setShowPast] = useState(false);
  const eventsQuery = useManageEventsQuery(showPast, currentPage, EVENTS_PER_PAGE);
  const events = eventsQuery.data?.events ?? [];
  const total = eventsQuery.data?.total ?? 0;
  const isLoading = eventsQuery.isLoading || eventsQuery.isFetching;

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [editingEvent, setEditingEvent] = useState<Event | null>(null);

  const [announcementState, setAnnouncementState] = useState<
    Record<string, 'loading' | 'success' | 'error'>
  >({});
  const [confirmAnnouncementEvent, setConfirmAnnouncementEvent] = useState<Event | null>(null);
  const [eventPendingDelete, setEventPendingDelete] = useState<Event | null>(null);

  const doSendAnnouncement = async (event: Event) => {
    setAnnouncementState((prev) => ({ ...prev, [event.id]: 'loading' }));
    try {
      await api.sendEventAnnouncement(event.id);
      setAnnouncementState((prev) => ({ ...prev, [event.id]: 'success' }));
      setTimeout(() => {
        setAnnouncementState((prev) => {
          const next = { ...prev };
          delete next[event.id];
          return next;
        });
      }, 3000);
    } catch (error) {
      console.error('Error sending announcement:', error);
      setAnnouncementState((prev) => ({ ...prev, [event.id]: 'error' }));
      setTimeout(() => {
        setAnnouncementState((prev) => {
          const next = { ...prev };
          delete next[event.id];
          return next;
        });
      }, 3000);
    }
  };

  const handleSendAnnouncement = (event: Event) => {
    setConfirmAnnouncementEvent(event);
  };

  const handleTogglePast = () => {
    setShowPast((p) => !p);
    setCurrentPage(1);
  };

  const handleRowClick = (event: Event) => {
    setEditingEvent(event);
  };

  const getEventTypeLabel = (type: number) => {
    const found = EVENT_TYPES.find((et) => et.value === type);
    return found ? t(found.labelKey) : String(type);
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr + 'T00:00:00').toLocaleDateString(i18n.language);
    } catch {
      return dateStr;
    }
  };

  const columns = [
    {
      key: 'name',
      header: t('common.name'),
      render: (event: Event) => (
        <div className="flex items-start gap-3 min-w-0">
          <EventAttachmentLead event={event} isSelected={false} />
          <div className="min-w-0">
            <p className="font-medium text-gray-900 dark:text-white">{event.name}</p>
            {event.description && (
              <p className="text-sm text-gray-500 dark:text-gray-400 truncate max-w-xs">
                {event.description}
              </p>
            )}
          </div>
        </div>
      ),
    },
    {
      key: 'eventType',
      header: t('events.type'),
      render: (event: Event) => (
        <span
          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
            EVENT_TYPE_COLORS[event.eventType] ?? EVENT_TYPE_COLORS[6]
          }`}
        >
          <Tag className="h-3 w-3" />
          {getEventTypeLabel(event.eventType)}
        </span>
      ),
    },
    {
      key: 'eventDate',
      header: t('events.date'),
      render: (event: Event) => (
        <div className="flex items-center gap-1.5 text-sm text-gray-700 dark:text-gray-300">
          <Calendar className="h-4 w-4 text-gray-400 flex-shrink-0" />
          <span>{formatDate(event.eventDate)}</span>
          {(event.startTime || event.endTime) && (
            <span className="flex items-center gap-1 text-gray-500 dark:text-gray-400">
              <Clock className="h-3.5 w-3.5" />
              {event.startTime && event.endTime
                ? `${event.startTime}–${event.endTime}`
                : event.startTime || event.endTime}
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'attendees',
      header: t('events.attendees'),
      render: (event: Event) =>
        event.attendeesCount != null ? (
          <div className="flex items-center gap-1 text-sm text-gray-700 dark:text-gray-300">
            <Users className="h-4 w-4 text-gray-400" />
            <span>{event.attendeesCount}</span>
            {event.studentsCount != null && (
              <span className="text-gray-500 dark:text-gray-400">
                {' '}+ {event.studentsCount} {t('events.students')}
              </span>
            )}
          </div>
        ) : (
          <span className="text-gray-400 text-sm">—</span>
        ),
    },
    {
      key: 'publicType',
      header: t('events.targetPublicLabel'),
      render: (event: Event) => (
        <span className="text-sm text-gray-700 dark:text-gray-300">
          {eventAudienceDisplayLabel(event, publicTypes, t('events.targetPublic.all'), t)}
        </span>
      ),
    },
    {
      key: 'actions',
      header: '',
      align: 'right' as const,
      render: (event: Event) => {
        const state = announcementState[event.id];
        return (
          <div
            className="flex justify-end"
            data-row-action
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
          <button
            type="button"
            onClick={() => handleSendAnnouncement(event)}
            disabled={state === 'loading'}
            title={t('events.sendAnnouncement')}
            className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors whitespace-nowrap ${
              state === 'success'
                ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300'
                : state === 'error'
                  ? 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300'
                  : 'bg-indigo-100 text-indigo-700 hover:bg-indigo-200 dark:bg-indigo-900/40 dark:text-indigo-300 dark:hover:bg-indigo-900/60'
            } disabled:opacity-50 disabled:cursor-not-allowed`}
          >
            {state === 'loading' ? (
              <div className="h-3.5 w-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
            ) : state === 'success' ? (
              <CheckCircle className="h-3.5 w-3.5" />
            ) : state === 'error' ? (
              <AlertCircle className="h-3.5 w-3.5" />
            ) : (
              <Mail className="h-3.5 w-3.5" />
            )}
            <span>
              {state === 'success'
                ? t('events.announcementSent')
                : state === 'error'
                  ? t('common.error')
                  : t('events.sendAnnouncement')}
            </span>
          </button>
          </div>
        );
      },
    },
  ];

  const totalPages = Math.ceil(total / EVENTS_PER_PAGE);

  return (
    <div className="space-y-4">
      <Link
        to="/events"
        className="inline-flex items-center gap-1.5 text-sm font-medium text-amber-600 dark:text-amber-400 hover:underline"
      >
        <ChevronLeft className="h-4 w-4" />
        {t('events.backToPublic')}
      </Link>
      <PageHeading
        icon={CalendarDays}
        title={t('events.title')}
        subtitle={showPast ? t('events.pastEvents') : t('events.currentEvents')}
        actions={
          <>
            <Button
              variant="secondary"
              onClick={handleTogglePast}
              leftIcon={showPast ? <ChevronLeft className="h-4 w-4" /> : <History className="h-4 w-4" />}
            >
              {showPast ? t('events.showCurrent') : t('events.showPast')}
            </Button>
            <Button onClick={() => setShowCreateModal(true)} leftIcon={<Plus className="h-4 w-4" />}>
              {t('events.newEvent')}
            </Button>
          </>
        }
      />

      {/* Events table */}
      <Card padding="none" className="flex flex-col min-h-0">
        <ListResultCountBar>{t('events.count', { count: total })}</ListResultCountBar>
        <ScrollableListRegion aria-label={t('events.title')}>
          <ResponsiveRecordList
            desktop={
              <Table
                columns={columns}
                data={events}
                keyExtractor={(event) => event.id}
                onRowClick={handleRowClick}
                isLoading={isLoading}
                emptyMessage={showPast ? t('events.noPastEvents') : t('events.noCurrentEvents')}
              />
            }
            mobile={
              isLoading && events.length === 0 ? (
                <Table
                  columns={columns}
                  data={[]}
                  keyExtractor={(event) => event.id}
                  isLoading
                  emptyMessage={showPast ? t('events.noPastEvents') : t('events.noCurrentEvents')}
                />
              ) : events.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                  {showPast ? t('events.noPastEvents') : t('events.noCurrentEvents')}
                </div>
              ) : (
                <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                  {events.map((event) => {
                    const state = announcementState[event.id];
                    return (
                      <button
                        key={event.id}
                        type="button"
                        className="w-full text-left border-b border-gray-100 dark:border-gray-800 last:border-b-0 p-4 space-y-2 hover:bg-gray-50 dark:hover:bg-gray-800/50"
                        onClick={() => handleRowClick(event)}
                      >
                        <div className="flex items-start gap-3 min-w-0">
                          <EventAttachmentLead event={event} isSelected={false} />
                          <div className="min-w-0 flex-1 space-y-1">
                            <p className="font-medium text-gray-900 dark:text-white">{event.name}</p>
                            {event.description ? (
                              <p className="text-sm text-gray-500 dark:text-gray-400 line-clamp-2">{event.description}</p>
                            ) : null}
                            <span
                              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${
                                EVENT_TYPE_COLORS[event.eventType] ?? EVENT_TYPE_COLORS[6]
                              }`}
                            >
                              <Tag className="h-3 w-3" />
                              {getEventTypeLabel(event.eventType)}
                            </span>
                          </div>
                        </div>
                        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-gray-600 dark:text-gray-300">
                          <span className="inline-flex items-center gap-1">
                            <Calendar className="h-4 w-4 text-gray-400 shrink-0" />
                            {formatDate(event.eventDate)}
                          </span>
                          {(event.startTime || event.endTime) && (
                            <span className="inline-flex items-center gap-1 text-gray-500 dark:text-gray-400">
                              <Clock className="h-3.5 w-3.5" />
                              {event.startTime && event.endTime
                                ? `${event.startTime}–${event.endTime}`
                                : event.startTime || event.endTime}
                            </span>
                          )}
                        </div>
                        <p className="text-sm text-gray-600 dark:text-gray-300">
                          {eventAudienceDisplayLabel(event, publicTypes, t('events.targetPublic.all'), t)}
                        </p>
                        <div
                          className="pt-1"
                          onClick={(e) => e.stopPropagation()}
                          onKeyDown={(e) => e.stopPropagation()}
                        >
                          <button
                            type="button"
                            onClick={() => handleSendAnnouncement(event)}
                            disabled={state === 'loading'}
                            className={`inline-flex min-h-[2.75rem] items-center gap-1.5 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                              state === 'success'
                                ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300'
                                : state === 'error'
                                  ? 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300'
                                  : 'bg-indigo-100 text-indigo-700 dark:bg-indigo-900/40 dark:text-indigo-300'
                            } disabled:opacity-50`}
                          >
                            {state === 'loading' ? (
                              <div className="h-3.5 w-3.5 border-2 border-current border-t-transparent rounded-full animate-spin" />
                            ) : state === 'success' ? (
                              <CheckCircle className="h-3.5 w-3.5" />
                            ) : state === 'error' ? (
                              <AlertCircle className="h-3.5 w-3.5" />
                            ) : (
                              <Mail className="h-3.5 w-3.5" />
                            )}
                            {state === 'success'
                              ? t('events.announcementSent')
                              : state === 'error'
                                ? t('common.error')
                                : t('events.sendAnnouncement')}
                          </button>
                        </div>
                      </button>
                    );
                  })}
                </div>
              )
            }
          />
        </ScrollableListRegion>
        {totalPages > 1 && (
          <div className="p-4 border-t border-gray-200 dark:border-gray-800 flex-shrink-0">
            <Pagination
              currentPage={currentPage}
              totalPages={totalPages}
              onPageChange={setCurrentPage}
            />
          </div>
        )}
      </Card>

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

      <AnnouncementSendDialog
        event={confirmAnnouncementEvent}
        isSending={
          confirmAnnouncementEvent != null &&
          announcementState[confirmAnnouncementEvent.id] === 'loading'
        }
        onClose={() => setConfirmAnnouncementEvent(null)}
        onConfirm={(event) => {
          void doSendAnnouncement(event).finally(() => {
            setConfirmAnnouncementEvent((current) => (current?.id === event.id ? null : current));
          });
        }}
      />

    </div>
  );
}
