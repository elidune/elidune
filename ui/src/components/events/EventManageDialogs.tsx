import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { Trash2 } from 'lucide-react';
import { Button, ConfirmDialog, Modal } from '@/components/common';
import EventForm from '@/components/events/EventForm';
import { invalidateEventListCaches } from '@/hooks/events/useManageEventsQuery';
import api from '@/services/api';
import type { Event } from '@/types';

export interface EventManageDialogsProps {
  showCreate: boolean;
  onCloseCreate: () => void;
  editingEvent: Event | null;
  onCloseEdit: () => void;
  eventPendingDelete: Event | null;
  onCloseDeleteConfirm: () => void;
  onRequestDelete: (event: Event) => void;
}

export default function EventManageDialogs({
  showCreate,
  onCloseCreate,
  editingEvent,
  onCloseEdit,
  eventPendingDelete,
  onCloseDeleteConfirm,
  onRequestDelete,
}: EventManageDialogsProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [isFormLoading, setIsFormLoading] = useState(false);

  const afterMutate = () => void invalidateEventListCaches(queryClient);

  const confirmDelete = async () => {
    const ev = eventPendingDelete;
    onCloseDeleteConfirm();
    if (!ev) return;
    try {
      await api.deleteEvent(ev.id);
      onCloseEdit();
      await afterMutate();
    } catch (error) {
      console.error('Error deleting event:', error);
    }
  };

  return (
    <>
      <Modal
        isOpen={showCreate}
        onClose={onCloseCreate}
        title={t('events.newEvent')}
        size="lg"
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="secondary" onClick={onCloseCreate}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" form="create-event-form" isLoading={isFormLoading}>
              {t('common.create')}
            </Button>
          </div>
        }
      >
        <EventForm
          formId="create-event-form"
          onLoadingChange={setIsFormLoading}
          onSuccess={() => {
            onCloseCreate();
            afterMutate();
          }}
        />
      </Modal>

      <ConfirmDialog
        isOpen={eventPendingDelete !== null}
        onClose={onCloseDeleteConfirm}
        onConfirm={() => void confirmDelete()}
        message={
          eventPendingDelete ? t('events.confirmDelete', { name: eventPendingDelete.name }) : ''
        }
        confirmVariant="danger"
        stackOnTop
      />

      <Modal
        isOpen={!!editingEvent}
        onClose={onCloseEdit}
        title={t('events.editEvent')}
        size="lg"
        footer={
          <div className="flex justify-between gap-2">
            <Button
              variant="danger"
              onClick={() => {
                if (!editingEvent) return;
                onRequestDelete(editingEvent);
              }}
              leftIcon={<Trash2 className="h-4 w-4" />}
            >
              {t('common.delete')}
            </Button>
            <div className="flex gap-2">
              <Button variant="secondary" onClick={onCloseEdit}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" form="edit-event-form" isLoading={isFormLoading}>
                {t('common.save')}
              </Button>
            </div>
          </div>
        }
      >
        {editingEvent && (
          <EventForm
            formId="edit-event-form"
            initialValues={editingEvent}
            onLoadingChange={setIsFormLoading}
            onSuccess={() => {
              onCloseEdit();
              afterMutate();
            }}
          />
        )}
      </Modal>
    </>
  );
}
