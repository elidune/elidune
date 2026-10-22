import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import MultiSelect, { type MultiSelectOption } from '@/components/common/MultiSelect';
import {
  EVENT_ALL_AUDIENCES_VALUE,
  eventAudienceFromMultiSelect,
  eventAudienceMultiSelectValue,
  type EventAudienceSelection,
} from '@/utils/eventPublicType';

export interface EventAudienceFieldsProps {
  id: string;
  selection: EventAudienceSelection;
  options: MultiSelectOption[];
  onSelectionChange: (next: EventAudienceSelection) => void;
  error?: string;
  note?: string;
}

export default function EventAudienceFields({
  id,
  selection,
  options,
  onSelectionChange,
  error,
  note,
}: EventAudienceFieldsProps) {
  const { t } = useTranslation();

  const selectOptions = useMemo(
    () => [
      { value: EVENT_ALL_AUDIENCES_VALUE, label: t('events.targetPublic.selectAll') },
      ...options,
    ],
    [options, t],
  );

  return (
    <MultiSelect
      id={id}
      label={t('events.targetPublicLabel')}
      placeholder={t('events.targetPublicPlaceholder')}
      hint={t('events.targetPublicHint')}
      note={note}
      error={error}
      required
      exclusiveValue={EVENT_ALL_AUDIENCES_VALUE}
      options={selectOptions}
      value={eventAudienceMultiSelectValue(selection)}
      onChange={(next) => onSelectionChange(eventAudienceFromMultiSelect(next, selection))}
    />
  );
}
