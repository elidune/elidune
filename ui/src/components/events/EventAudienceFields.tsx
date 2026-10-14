import { useTranslation } from 'react-i18next';
import MultiSelect, { type MultiSelectOption } from '@/components/common/MultiSelect';
import { formChoiceControlClass, formChoiceLabelClass } from '@/utils/formControl';
import {
  applyAllAudiencesToggle,
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
  const allAudiences = selection.allAudiences === true;

  return (
    <div className="space-y-2">
      <label className={formChoiceLabelClass()} htmlFor={`${id}-all`}>
        <input
          id={`${id}-all`}
          type="checkbox"
          checked={allAudiences}
          className={formChoiceControlClass('rounded')}
          onChange={(event) => {
            onSelectionChange(applyAllAudiencesToggle(selection, event.target.checked));
          }}
        />
        {t('events.targetPublic.all')}
      </label>
      <MultiSelect
        id={id}
        label={t('events.targetPublicLabel')}
        placeholder={t('events.targetPublicPlaceholder')}
        hint={t('events.targetPublicHint')}
        note={note}
        error={error}
        required={!allAudiences}
        disabled={allAudiences}
        options={options}
        value={allAudiences ? [] : [...selection.publicTypes]}
        onChange={(next) => {
          onSelectionChange({ allAudiences: false, publicTypes: next });
        }}
      />
    </div>
  );
}
