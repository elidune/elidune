import { useTranslation } from 'react-i18next';
import { Plus, Save, Trash2 } from 'lucide-react';
import Button from '@/components/common/Button';
import Card, { CardHeader } from '@/components/common/Card';
import Input from '@/components/common/Input';
import { formControlClass, formLabelClass } from '@/utils/formControl';
import type { AccountTypeDefinition } from '@/types';
import {
  ACCOUNT_TYPE_RIGHT_FIELDS,
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
  const rightSelectClass = formControlClass({
    className: 'min-w-[4.5rem] max-w-[6rem] px-1.5 text-xs',
  });

  return (
    <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
      <CardHeader title={t('settings.accountTypes.title')} />
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
        <p role="alert" className="text-sm text-red-700 dark:text-red-300 bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 rounded-lg px-3 py-2 mb-4 mx-4">
          {rowError}
        </p>
      ) : null}

      {canEdit ? (
        <div className="px-4 pb-4">
          {createOpen ? (
            <form
              className="rounded-xl border border-gray-200 dark:border-gray-700 p-3 space-y-3"
              onSubmit={(event) => {
                event.preventDefault();
                onSubmitCreate();
              }}
            >
              <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('settings.accountTypes.create')}</h3>
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
              <div className="flex flex-wrap gap-2">
                {ACCOUNT_TYPE_RIGHT_FIELDS.map((field) => (
                  <label key={field} className="text-xs text-gray-600 dark:text-gray-400">
                    <span className="block mb-1">{t(`settings.accountTypes.rights.${field}`)}</span>
                    <select
                      value={normAccountTypeRight(createRights[field])}
                      onChange={(event) => onCreateRight(field, event.target.value)}
                      className={rightSelectClass}
                      aria-label={t(`settings.accountTypes.rights.${field}`)}
                    >
                      <option value="n">{t('settings.accountTypes.level.n')}</option>
                      {accountTypeRightAllowsOwn(field) ? (
                        <option value="o">{t('settings.accountTypes.level.o')}</option>
                      ) : null}
                      <option value="r">{t('settings.accountTypes.level.r')}</option>
                      <option value="w">{t('settings.accountTypes.level.w')}</option>
                    </select>
                  </label>
                ))}
              </div>
              {createFieldError ? (
                <p role="alert" className="text-sm text-red-600 dark:text-red-400">{createFieldError}</p>
              ) : null}
              {createError ? (
                <p role="alert" className="text-sm text-red-600 dark:text-red-400">{createError}</p>
              ) : null}
              <div className="flex justify-end gap-2">
                <Button type="button" variant="secondary" onClick={onCancelCreate} disabled={creating}>
                  {t('common.cancel')}
                </Button>
                <Button type="submit" isLoading={creating} leftIcon={<Plus className="h-4 w-4" />}>
                  {t('common.create')}
                </Button>
              </div>
            </form>
          ) : (
            <Button type="button" variant="secondary" leftIcon={<Plus className="h-4 w-4" />} onClick={onOpenCreate}>
              {t('settings.accountTypes.create')}
            </Button>
          )}
        </div>
      ) : null}

      {rows.length === 0 ? (
        <p className="text-center text-gray-500 dark:text-gray-400 py-8">{t('settings.accountTypes.empty')}</p>
      ) : (
        <div className="overflow-x-auto px-4 pb-4">
          <table className="w-full min-w-[68rem] text-sm">
            <thead>
              <tr className="border-b border-gray-200 dark:border-gray-700">
                <th className="text-left py-2 pr-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('settings.accountTypes.code')}
                </th>
                <th className="text-left py-2 pr-2 font-medium text-gray-600 dark:text-gray-400">
                  {t('settings.accountTypes.name')}
                </th>
                {ACCOUNT_TYPE_RIGHT_FIELDS.map((field) => (
                  <th
                    key={field}
                    title={t(`settings.accountTypes.rights.${field}`)}
                    className="text-left py-2 px-1 font-medium text-gray-600 dark:text-gray-400 whitespace-normal max-w-56"
                  >
                    {t(`settings.accountTypes.rights.${field}`)}
                  </th>
                ))}
                {canEdit ? (
                  <th className="py-2 pl-2 w-36 text-right font-medium text-gray-600 dark:text-gray-400">
                    {t('common.actions')}
                  </th>
                ) : null}
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
              {rows.map((row) => {
                const orig = originals.find((item) => item.code === row.code);
                const dirty = orig ? buildAccountTypeUpdate(orig, row) !== null : false;
                const builtIn = isBuiltInAccountType(row);
                const label = row.name?.trim() || row.code;
                return (
                  <tr key={row.code}>
                    <td className="py-2 pr-2">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-xs text-gray-700 dark:text-gray-300">{row.code}</span>
                        {builtIn ? (
                          <span className="inline-flex items-center rounded-full bg-gray-100 px-2 py-0.5 text-xs font-medium text-gray-700 ring-1 ring-gray-300/70 dark:bg-gray-800 dark:text-gray-200 dark:ring-gray-600">
                            {t('settings.accountTypes.builtIn')}
                          </span>
                        ) : null}
                      </div>
                    </td>
                    <td className="py-2 pr-2 min-w-[8rem]">
                      {canEdit ? (
                        <Input
                          value={row.name ?? ''}
                          onChange={(event) => onUpdateRow(row.code, { name: event.target.value })}
                          className="text-sm py-1.5"
                          aria-label={t('settings.accountTypes.name')}
                        />
                      ) : (
                        <span className="text-gray-900 dark:text-white">{row.name || '—'}</span>
                      )}
                    </td>
                    {ACCOUNT_TYPE_RIGHT_FIELDS.map((field) => (
                      <td key={field} className="py-2 px-1">
                        {canEdit ? (
                          <select
                            value={normAccountTypeRight(row[field])}
                            onChange={(event) => {
                              const value = event.target.value;
                              onUpdateRow(row.code, {
                                [field]: value === '' ? null : value,
                              } as Pick<AccountTypeDefinition, AccountTypeRightField>);
                            }}
                            className={rightSelectClass}
                            aria-label={t(`settings.accountTypes.rights.${field}`)}
                          >
                            <option value="">{t('settings.accountTypes.level.none')}</option>
                            <option value="n">{t('settings.accountTypes.level.n')}</option>
                            {(accountTypeRightAllowsOwn(field) ||
                              normAccountTypeRight(row[field]) === 'o') && (
                              <option value="o">{t('settings.accountTypes.level.o')}</option>
                            )}
                            <option value="r">{t('settings.accountTypes.level.r')}</option>
                            <option value="w">{t('settings.accountTypes.level.w')}</option>
                          </select>
                        ) : (
                          <span className="text-xs text-gray-700 dark:text-gray-300 font-mono">
                            {normAccountTypeRight(row[field]) || '—'}
                          </span>
                        )}
                      </td>
                    ))}
                    {canEdit ? (
                      <td className="py-2 pl-2 text-right">
                        <div className="flex justify-end gap-1">
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
                              size="icon"
                              variant="danger"
                              aria-label={t('settings.accountTypes.deleteNamed', { name: label })}
                              disabled={savingCode !== null}
                              onClick={() => onDeleteRow(row.code)}
                            >
                              <Trash2 className="h-3.5 w-3.5" />
                            </Button>
                          )}
                        </div>
                      </td>
                    ) : null}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  );
}

