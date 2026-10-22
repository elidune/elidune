import { useState, useEffect, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Input } from '@/components/common';
import EventAudienceFields from '@/components/events/EventAudienceFields';
import { EVENT_TYPES } from '@/components/events/eventDisplayConstants';
import api from '@/services/api';
import type { Event, CreateEvent, UpdateEvent } from '@/types';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import {
  eventAudienceErrorKey,
  eventSaveErrorMessage,
  resolveEventAudiences,
} from '@/utils/eventPublicType';
import { localizedPublicTypeLabel } from '@/utils/publicTypeDisplay';
import { CHILD_PUBLIC_TYPE_NAME } from '@/utils/legalGuardian';
import { fileToAttachmentInput, base64ToDataUrl, isImageMime } from '@/utils/eventAttachment';
import {
  formChoiceControlClass,
  formControlClass,
  formTextareaClass,
  formLabelClass,
  formChoiceLabelClass,
} from '@/utils/formControl';

export interface EventFormProps {
  formId: string;
  initialValues?: Event;
  onLoadingChange: (loading: boolean) => void;
  onSuccess: () => void;
}

export default function EventForm({
  formId,
  initialValues,
  onLoadingChange,
  onSuccess,
}: EventFormProps) {
  const { t } = useTranslation();
  const { data: publicTypes = [] } = usePublicTypesQuery();
  const isEdit = !!initialValues;
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [formData, setFormData] = useState({
    name: initialValues?.name ?? '',
    eventDate: initialValues?.eventDate ?? '',
    eventType: initialValues?.eventType != null ? String(initialValues.eventType) : '0',
    startTime: initialValues?.startTime ?? '',
    endTime: initialValues?.endTime ?? '',
    description: initialValues?.description ?? '',
    partnerName: initialValues?.partnerName ?? '',
    schoolName: initialValues?.schoolName ?? '',
    className: initialValues?.className ?? '',
    attendeesCount:
      initialValues?.attendeesCount != null ? String(initialValues.attendeesCount) : '',
    studentsCount:
      initialValues?.studentsCount != null ? String(initialValues.studentsCount) : '',
    allAudiences: initialValues?.allAudiences === true,
    publicTypes: initialValues?.allAudiences ? [] : [...(initialValues?.publicTypes ?? [])],
    notes: initialValues?.notes ?? '',
  });
  const [audienceError, setAudienceError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const [attachmentFile, setAttachmentFile] = useState<File | null>(null);
  const [removeAttachment, setRemoveAttachment] = useState(false);

  const { data: fullEvent } = useQuery({
    queryKey: ['event-edit', initialValues?.id],
    queryFn: () => api.getEvent(initialValues!.id),
    enabled: isEdit && !!initialValues?.id,
    staleTime: 5 * 60 * 1000,
  });

  const attachmentSource = fullEvent ?? initialValues;
  const previewMime = attachmentFile?.type ?? attachmentSource?.attachmentMimeType ?? '';
  const previewIsImage = attachmentFile
    ? isImageMime(attachmentFile.type)
    : isImageMime(previewMime);

  const attachmentPreviewUrl = useMemo(() => {
    if (attachmentFile && isImageMime(attachmentFile.type)) return URL.createObjectURL(attachmentFile);
    return null;
  }, [attachmentFile]);

  useEffect(() => {
    return () => {
      if (attachmentPreviewUrl) URL.revokeObjectURL(attachmentPreviewUrl);
    };
  }, [attachmentPreviewUrl]);

  const [prevEventId, setPrevEventId] = useState(initialValues?.id);
  if (initialValues?.id !== prevEventId) {
    setPrevEventId(initialValues?.id);
    setAttachmentFile(null);
    setRemoveAttachment(false);
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const audience = resolveEventAudiences({
      allAudiences: formData.allAudiences,
      publicTypes: formData.publicTypes,
    });
    if (!audience.ok) {
      setSaveError(null);
      setAudienceError(t(eventAudienceErrorKey(audience.reason)));
      return;
    }
    setAudienceError(null);
    setSaveError(null);
    onLoadingChange(true);
    try {
      const common = {
        name: formData.name || undefined,
        eventDate: formData.eventDate || undefined,
        eventType: formData.eventType !== '' ? Number(formData.eventType) : undefined,
        startTime: formData.startTime || null,
        endTime: formData.endTime || null,
        description: formData.description || null,
        partnerName: formData.partnerName || null,
        schoolName: formData.schoolName || null,
        className: formData.className || null,
        attendeesCount: formData.attendeesCount !== '' ? Number(formData.attendeesCount) : null,
        studentsCount: formData.studentsCount !== '' ? Number(formData.studentsCount) : null,
        allAudiences: audience.payload.allAudiences,
        publicTypes: audience.payload.publicTypes,
        notes: formData.notes || null,
      };

      if (isEdit && initialValues) {
        const u: UpdateEvent = { ...common };
        if (removeAttachment) u.removeAttachment = true;
        else if (attachmentFile) u.attachment = await fileToAttachmentInput(attachmentFile);
        await api.updateEvent(initialValues.id, u);
      } else {
        const c: CreateEvent = {
          name: formData.name,
          eventDate: formData.eventDate,
          eventType: common.eventType,
          startTime: common.startTime,
          endTime: common.endTime,
          description: common.description,
          partnerName: common.partnerName,
          schoolName: common.schoolName,
          className: common.className,
          attendeesCount: common.attendeesCount,
          studentsCount: common.studentsCount,
          allAudiences: audience.payload.allAudiences,
          publicTypes: audience.payload.publicTypes,
          notes: common.notes,
        };
        if (attachmentFile) c.attachment = await fileToAttachmentInput(attachmentFile);
        await api.createEvent(c);
      }
      onSuccess();
    } catch (error) {
      setSaveError(eventSaveErrorMessage(error, t));
    } finally {
      onLoadingChange(false);
    }
  };

  const schoolVisit = formData.eventType === '1';
  const childSelected = formData.publicTypes.includes(CHILD_PUBLIC_TYPE_NAME);
  const guardianNote = formData.allAudiences
    ? t('events.targetPublic.allChildGuardianNote')
    : childSelected
      ? t('events.targetPublic.childGuardianHint')
      : undefined;
  const audienceOptions = publicTypes.map((pt) => ({
    value: pt.name,
    label: localizedPublicTypeLabel(t, pt),
  }));

  return (
    <form id={formId} onSubmit={handleSubmit} className="space-y-4">
      {saveError && (
        <p role="alert" className="text-sm text-red-600 dark:text-red-400">
          {saveError}
        </p>
      )}
      <h4 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider">
        {t('events.basicInfo')}
      </h4>
      <Input
        label={t('common.name')}
        value={formData.name}
        onChange={(e) => setFormData({ ...formData, name: e.target.value })}
        required={!isEdit}
      />
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <Input
          label={t('events.date')}
          type="date"
          value={formData.eventDate}
          onChange={(e) => setFormData({ ...formData, eventDate: e.target.value })}
          required={!isEdit}
        />
        <div>
          <label className={formLabelClass()}>{t('events.type')}</label>
          <select
            value={formData.eventType}
            onChange={(e) => setFormData({ ...formData, eventType: e.target.value })}
            className={formControlClass({ className: 'w-full' })}
          >
            {EVENT_TYPES.map((et) => (
              <option key={et.value} value={String(et.value)}>
                {t(et.labelKey)}
              </option>
            ))}
          </select>
        </div>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
        <Input
          label={t('events.startTime')}
          type="time"
          value={formData.startTime}
          onChange={(e) => setFormData({ ...formData, startTime: e.target.value })}
        />
        <Input
          label={t('events.endTime')}
          type="time"
          value={formData.endTime}
          onChange={(e) => setFormData({ ...formData, endTime: e.target.value })}
        />
      </div>

      <div>
        <label className={formLabelClass()}>{t('common.description')}</label>
        <textarea
          value={formData.description}
          onChange={(e) => setFormData({ ...formData, description: e.target.value })}
          rows={2}
          className={formTextareaClass({ className: 'resize-none' })}
        />
      </div>

      <h4 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider pt-2">
        {t('events.audienceInfo')}
      </h4>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 items-start">
        <EventAudienceFields
          id={`${formId}-audience`}
          selection={{ allAudiences: formData.allAudiences, publicTypes: formData.publicTypes }}
          options={audienceOptions}
          note={guardianNote}
          error={audienceError ?? undefined}
          onSelectionChange={(next) => {
            setAudienceError(null);
            setSaveError(null);
            setFormData((current) => ({
              ...current,
              allAudiences: next.allAudiences,
              publicTypes: [...next.publicTypes],
            }));
          }}
        />
        <Input
          label={t('events.attendeesCount')}
          type="number"
          min="0"
          value={formData.attendeesCount}
          onChange={(e) => setFormData({ ...formData, attendeesCount: e.target.value })}
        />
      </div>
      <Input
        label={t('events.partnerName')}
        value={formData.partnerName}
        onChange={(e) => setFormData({ ...formData, partnerName: e.target.value })}
      />

      {schoolVisit && (
        <>
          <h4 className="text-sm font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wider pt-2">
            {t('events.schoolInfo')}
          </h4>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <Input
              label={t('events.schoolName')}
              value={formData.schoolName}
              onChange={(e) => setFormData({ ...formData, schoolName: e.target.value })}
            />
            <Input
              label={t('events.className')}
              value={formData.className}
              onChange={(e) => setFormData({ ...formData, className: e.target.value })}
            />
          </div>
          <Input
            label={t('events.studentsCount')}
            type="number"
            min="0"
            value={formData.studentsCount}
            onChange={(e) => setFormData({ ...formData, studentsCount: e.target.value })}
          />
        </>
      )}

      <div className="space-y-2">
        <span className={formLabelClass({ marginBottom: false })}>
          {t('events.flyerOrAttachment')}
        </span>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*,application/pdf"
          className={formControlClass({
            autoHeight: true,
            className:
              'w-full cursor-pointer text-sm text-gray-600 file:mr-3 file:rounded-md file:border-0 file:bg-amber-50 file:px-3 file:py-1 file:text-sm file:font-medium file:text-amber-800 hover:file:bg-amber-100 dark:text-gray-400 dark:file:bg-amber-900/40 dark:file:text-amber-200 dark:hover:file:bg-amber-900/60',
          })}
          onChange={(e) => {
            const f = e.target.files?.[0];
            setAttachmentFile(f ?? null);
            setRemoveAttachment(false);
          }}
        />
        {isEdit &&
          (attachmentSource?.attachmentFileName || attachmentSource?.attachmentDataBase64) && (
            <label className={formChoiceLabelClass()}>
              <input
                type="checkbox"
                checked={removeAttachment}
                className={formChoiceControlClass('rounded')}
                onChange={(e) => {
                  setRemoveAttachment(e.target.checked);
                  if (e.target.checked) {
                    setAttachmentFile(null);
                    if (fileInputRef.current) fileInputRef.current.value = '';
                  }
                }}
              />
              {t('events.removeAttachment')}
            </label>
          )}
        {!removeAttachment &&
          previewIsImage &&
          (attachmentPreviewUrl ||
            (attachmentSource?.attachmentDataBase64 && !attachmentFile)) && (
            <div className="mt-1">
              <img
                src={
                  attachmentPreviewUrl ??
                  (attachmentSource?.attachmentDataBase64 && previewMime
                    ? base64ToDataUrl(attachmentSource.attachmentDataBase64, previewMime)
                    : '')
                }
                alt=""
                className="max-h-36 rounded-lg border border-gray-200 dark:border-gray-700 object-contain bg-gray-50 dark:bg-gray-950"
              />
            </div>
          )}
        {!removeAttachment && attachmentFile && !previewIsImage && (
          <p className="text-sm text-gray-600 dark:text-gray-400">{attachmentFile.name}</p>
        )}
        {!removeAttachment &&
          !attachmentFile &&
          !previewIsImage &&
          attachmentSource?.attachmentFileName &&
          (attachmentSource.attachmentDataBase64 || attachmentSource.attachmentFileName) && (
            <p className="text-sm text-gray-600 dark:text-gray-400">
              {attachmentSource.attachmentFileName}
            </p>
          )}
      </div>

      <div>
        <label className={formLabelClass()}>{t('users.notes')}</label>
        <textarea
          value={formData.notes}
          onChange={(e) => setFormData({ ...formData, notes: e.target.value })}
          rows={2}
          className={formTextareaClass({ className: 'resize-none' })}
        />
      </div>
    </form>
  );
}
