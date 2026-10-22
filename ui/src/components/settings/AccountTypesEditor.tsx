import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, Plus, Save, Shield, Trash2 } from 'lucide-react';
import Button from '@/components/common/Button';
import Card, { CardHeader } from '@/components/common/Card';
import Input from '@/components/common/Input';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import type { AccountTypeDefinition } from '@/types';
import {
  ACCOUNT_TYPE_RIGHT_GROUPS,
  accountTypeRightAllowsOwn,
  buildAccountTypeUpdate,
  isBuiltInAccountType,
  normAccountTypeRight,
  type AccountTypeRightField,
} from '@/utils/accountTypeEditor';

export interface AccountTypesEditorProps {
  rows: AccountTypeDefinition[];
  originals: AccountTypeDefinition[];
  canEdit: boolean;
  savingCode: string | null;
  rowError: string | null;
  banner: string | null;
  createOpen: boolean;
  createCode: string;
  createName: string;
  createRights: Record<AccountTypeRightField, string>;
  createFieldError: string | null;
  createError: string | null;
  creating: boolean;
  onUpdateRow: (code: string, patch: Partial<AccountTypeDefinition>) => void;
  onSaveRow: (code: string) => void;
  onDeleteRow: (code: string) => void;
  onOpenCreate: () => void;
  onCancelCreate: () => void;
  onCreateCode: (value: string) => void;
  onCreateName: (value: string) => void;
  onCreateRight: (field: AccountTypeRightField, value: string) => void;
  onSubmitCreate: () => void;
}

const ACCOUNT_TYPE_RIGHT_CELL_CLASS =
  'flex min-w-0 flex-1 basis-0 flex-col rounded-lg border border-gray-200/90 bg-white p-3 dark:border-gray-700 dark:bg-gray-900/40';

function levelLabelKey(value: string): 'none' | 'n' | 'o' | 'r' | 'w' {
  const norm = normAccountTypeRight(value);
  if (!norm) return 'none';
  return norm as 'n' | 'o' | 'r' | 'w';
}

function AccountTypeRightLevelSelect({
  id,
  field,
  value,
  disabled,
  onChange,
  allowUnset,
}: {
  id: string;
  field: AccountTypeRightField;
  value: string | null | undefined;
  disabled?: boolean;
  onChange: (next: string) => void;
  allowUnset?: boolean;
}) {
  const { t } = useTranslation();
  const norm = normAccountTypeRight(value);
  const selectClass = formControlClass({ className: 'w-full text-sm' });

  return (
    <select
      id={id}
      value={norm}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
      className={selectClass}
      aria-label={t(`settings.accountTypes.rights.${field}`)}
    >
      {allowUnset ? <option value="">{t('settings.accountTypes.level.none')}</option> : null}
      <option value="n">{t('settings.accountTypes.level.n')}</option>
      {(accountTypeRightAllowsOwn(field) || norm === 'o') && (
        <option value="o">{t('settings.accountTypes.level.o')}</option>
      )}
      <option value="r">{t('settings.accountTypes.level.r')}</option>
      <option value="w">{t('settings.accountTypes.level.w')}</option>
    </select>
  );
}

function AccountTypeRightsPanel({
  rightsSource,
  canEdit,
  allowUnset,
  onRightChange,
}: {
  rightsSource: AccountTypeDefinition | Record<AccountTypeRightField, string>;
  canEdit: boolean;
  allowUnset?: boolean;
  onRightChange?: (field: AccountTypeRightField, value: string) => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="space-y-5">
      {ACCOUNT_TYPE_RIGHT_GROUPS.map((group) => (
        <section key={group.key} aria-labelledby={`account-type-group-${group.key}`}>
          <h4
            id={`account-type-group-${group.key}`}
            className="mb-3 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400"
          >
            {t(`settings.accountTypes.groups.${group.key}`)}
          </h4>
          <ul className="flex w-full list-none gap-3">
            {group.fields.map((field) => {
              const raw = rightsSource[field];
              const fieldId = `account-type-right-${field}`;
              const rightsLabel = t(`settings.accountTypes.rights.${field}`);
              return (
                <li key={field} className={ACCOUNT_TYPE_RIGHT_CELL_CLASS}>
                  <label
                    htmlFor={canEdit ? fieldId : undefined}
                    className={`${formLabelClass()} min-w-0 truncate`}
                    title={rightsLabel}
                  >
                    {rightsLabel}
                  </label>
                  {canEdit && onRightChange ? (
                    <AccountTypeRightLevelSelect
                      id={fieldId}
                      field={field}
                      value={typeof raw === 'string' ? raw : null}
                      allowUnset={allowUnset}
                      onChange={(next) => onRightChange(field, next)}
                    />
                  ) : (
                    <p className="mt-1 text-sm font-medium text-gray-900 dark:text-gray-100">
                      {t(`settings.accountTypes.level.${levelLabelKey(typeof raw === 'string' ? raw : '')}`)}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}

export function AccountTypesEditor({
  rows,
  originals,
  canEdit,
  savingCode,
  rowError,
  banner,
  createOpen,
  createCode,
  createName,
  createRights,
  createFieldError,
  createError,
  creating,
  onUpdateRow,
  onSaveRow,
  onDeleteRow,
  onOpenCreate,
  onCancelCreate,
  onCreateCode,
  onCreateName,
  onCreateRight,
  onSubmitCreate,
}: AccountTypesEditorProps) {
  const { t } = useTranslation();
  const [expandedCode, setExpandedCode] = useState<string | null>(null);

  const toggleExpanded = (code: string) => {
    setExpandedCode((current) => (current === code ? null : code));
  };

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader
        title={t('settings.accountTypes.title')}
        action={
          canEdit && !createOpen ? (
            <Button type="button" size="sm" variant="primary" leftIcon={<Plus className="h-4 w-4" />} onClick={onOpenCreate}>
              {t('settings.accountTypes.create')}
            </Button>
          ) : null
        }
      />
      <p className="text-sm text-gray-600 dark:text-gray-400 px-4 -mt-2 mb-4">{t('settings.accountTypes.subtitle')}</p>

      {!canEdit ? (
        <p className="text-sm text-amber-800 dark:text-amber-200 bg-amber-50 dark:bg-amber-950/40 border border-amber-200 dark:border-amber-800 rounded-lg px-3 py-2 mb-4 mx-4">
          {t('settings.accountTypes.readOnlyHint')}
        </p>
      ) : null}

      {banner ? (
        <p className="text-sm text-indigo-800 dark:text-indigo-200 bg-indigo-50 dark:bg-indigo-950/40 border border-indigo-200 dark:border-indigo-800 rounded-lg px-3 py-2 mb-4 mx-4">
          {banner}
        </p>
      ) : null}

      {rowError ? (
        <p
          role="alert"
          className="text-sm text-red-700 dark:text-red-300 bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 rounded-lg px-3 py-2 mb-4 mx-4"
        >
          {rowError}
        </p>
      ) : null}

      {canEdit && createOpen ? (
        <div className="px-4 pb-4">
          <form
            className="rounded-xl border border-gray-200 dark:border-gray-700 bg-gray-50/80 dark:bg-gray-900/30 p-4 space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              onSubmitCreate();
            }}
          >
            <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{t('settings.accountTypes.create')}</h3>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label htmlFor="account-type-create-code" className={formLabelClass()}>
                  {t('settings.accountTypes.code')}
                </label>
                <Input
                  id="account-type-create-code"
                  value={createCode}
                  onChange={(event) => onCreateCode(event.target.value)}
                  placeholder={t('settings.accountTypes.codePlaceholder')}
                  autoComplete="off"
                />
              </div>
              <div>
                <label htmlFor="account-type-create-name" className={formLabelClass()}>
                  {t('settings.accountTypes.name')}
                </label>
                <Input
                  id="account-type-create-name"
                  value={createName}
                  onChange={(event) => onCreateName(event.target.value)}
                />
              </div>
            </div>
            <AccountTypeRightsPanel
              rightsSource={createRights}
              canEdit
              onRightChange={(field, value) => onCreateRight(field, value)}
            />
            {createFieldError ? (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400">
                {createFieldError}
              </p>
            ) : null}
            {createError ? (
              <p role="alert" className="text-sm text-red-600 dark:text-red-400">
                {createError}
              </p>
            ) : null}
            <div className="flex justify-end gap-2 pt-1">
              <Button type="button" variant="secondary" onClick={onCancelCreate} disabled={creating}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" isLoading={creating} leftIcon={<Plus className="h-4 w-4" />}>
                {t('common.create')}
              </Button>
            </div>
          </form>
        </div>
      ) : null}

      {rows.length === 0 ? (
        <p className="text-center text-gray-500 dark:text-gray-400 py-8">{t('settings.accountTypes.empty')}</p>
      ) : (
        <div className="px-4 pb-4 space-y-3">
          {rows.map((row) => {
            const orig = originals.find((item) => item.code === row.code);
            const dirty = orig ? buildAccountTypeUpdate(orig, row) !== null : false;
            const builtIn = isBuiltInAccountType(row);
            const label = row.name?.trim() || row.code;
            const expanded = expandedCode === row.code;

            return (
              <article
                key={row.code}
                className="overflow-hidden rounded-xl border border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-800/50"
              >
                <div className="flex items-stretch">
                  <button
                    type="button"
                    className="flex min-w-0 flex-1 items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 dark:hover:bg-gray-800/80"
                    aria-expanded={expanded}
                    onClick={() => toggleExpanded(row.code)}
                  >
                    <Shield className="h-5 w-5 shrink-0 text-indigo-500 dark:text-indigo-400" aria-hidden />
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="font-medium text-gray-900 dark:text-white">{label}</span>
                        <code className="rounded-md bg-gray-100 px-1.5 py-0.5 font-mono text-xs text-gray-600 dark:bg-gray-900 dark:text-gray-300">
                          {row.code}
                        </code>
                        {builtIn ? (
                          <span className="inline-flex items-center rounded-full bg-gray-100 px-2 py-0.5 text-xs font-medium text-gray-700 ring-1 ring-gray-300/70 dark:bg-gray-800 dark:text-gray-200 dark:ring-gray-600">
                            {t('settings.accountTypes.builtIn')}
                          </span>
                        ) : null}
                        {dirty && canEdit ? (
                          <span className="inline-flex items-center rounded-full bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-800 ring-1 ring-amber-200/80 dark:bg-amber-950/50 dark:text-amber-200 dark:ring-amber-800">
                            {t('settings.accountTypes.unsavedChanges')}
                          </span>
                        ) : null}
                      </div>
                    </div>
                    <ChevronDown
                      className={`h-5 w-5 shrink-0 text-gray-400 transition-transform ${expanded ? 'rotate-180' : ''}`}
                      aria-hidden
                    />
                  </button>
                </div>

                {expanded ? (
                  <div className="border-t border-gray-200 bg-gray-50/90 px-4 py-4 dark:border-gray-700 dark:bg-gray-900/40">
                    {canEdit ? (
                      <div className="mb-4 max-w-md">
                        <Input
                          label={t('settings.accountTypes.name')}
                          value={row.name ?? ''}
                          onChange={(event) => onUpdateRow(row.code, { name: event.target.value })}
                        />
                      </div>
                    ) : null}
                    <AccountTypeRightsPanel
                      rightsSource={row}
                      canEdit={canEdit}
                      allowUnset={canEdit}
                      onRightChange={
                        canEdit
                          ? (field, value) => {
                              onUpdateRow(row.code, {
                                [field]: value === '' ? null : value,
                              } as Pick<AccountTypeDefinition, AccountTypeRightField>);
                            }
                          : undefined
                      }
                    />
                    {canEdit ? (
                      <div className="mt-5 flex flex-wrap justify-end gap-2 border-t border-gray-200 pt-4 dark:border-gray-700">
                        <Button
                          type="button"
                          size="sm"
                          variant="secondary"
                          disabled={!dirty || savingCode !== null}
                          isLoading={savingCode === row.code}
                          leftIcon={<Save className="h-3.5 w-3.5" />}
                          onClick={() => onSaveRow(row.code)}
                        >
                          {t('common.save')}
                        </Button>
                        {builtIn ? null : (
                          <Button
                            type="button"
                            size="sm"
                            variant="danger"
                            leftIcon={<Trash2 className="h-3.5 w-3.5" />}
                            aria-label={t('settings.accountTypes.deleteNamed', { name: label })}
                            disabled={savingCode !== null}
                            onClick={() => onDeleteRow(row.code)}
                          >
                            {t('common.delete')}
                          </Button>
                        )}
                      </div>
                    ) : null}
                  </div>
                ) : null}
              </article>
            );
          })}
        </div>
      )}
    </Card>
  );
}
