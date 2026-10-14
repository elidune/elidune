import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import axios from 'axios';
import { Card, ConfirmDialog } from '@/components/common';
import { useAuth } from '@/contexts/AuthContext';
import api from '@/services/api';
import { ACCOUNT_TYPES_QUERY_KEY, useAccountTypesQuery } from '@/hooks/useAccountTypesQuery';
import { getApiErrorMessage } from '@/utils/apiError';
import { canWriteSettings } from '@/types';
import type { AccountTypeDefinition } from '@/types';
import { AccountTypesEditor } from '@/components/settings/AccountTypesEditor';
import {
  accountTypeCodeError,
  accountTypeNameError,
  accountTypeUpdateChangesRights,
  blankAccountTypeRights,
  buildAccountTypeUpdate,
  buildCreateAccountTypeRequest,
} from '@/utils/accountTypeEditor';

function cloneTypes(list: AccountTypeDefinition[]): AccountTypeDefinition[] {
  return list.map((row) => ({ ...row }));
}

export default function AccountTypesSettings() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { user } = useAuth();
  const canEdit = canWriteSettings(user, api.getToken());

  const { data, isLoading, isError } = useAccountTypesQuery();
  const [rows, setRows] = useState<AccountTypeDefinition[]>(() =>
    data?.length ? cloneTypes(data) : []
  );
  const [savingCode, setSavingCode] = useState<string | null>(null);
  const [rowError, setRowError] = useState<string | null>(null);
  const [banner, setBanner] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [createCode, setCreateCode] = useState('');
  const [createName, setCreateName] = useState('');
  const [createRights, setCreateRights] = useState(blankAccountTypeRights);
  const [createFieldError, setCreateFieldError] = useState<string | null>(null);
  const [createError, setCreateError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<AccountTypeDefinition | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [syncedData, setSyncedData] = useState(data);
  if (data !== syncedData) {
    setSyncedData(data);
    if (data?.length) setRows(cloneTypes(data));
  }

  const showBanner = useCallback((message: string) => {
    setBanner(message);
    window.setTimeout(() => setBanner(null), 9000);
  }, []);

  const updateRow = (code: string, patch: Partial<AccountTypeDefinition>) => {
    setRows((prev) => prev.map((row) => (row.code === code ? { ...row, ...patch } : row)));
    setRowError(null);
  };

  const handleSaveRow = async (code: string) => {
    const orig = data?.find((row) => row.code === code);
    const cur = rows.find((row) => row.code === code);
    if (!orig || !cur) return;
    const body = buildAccountTypeUpdate(orig, cur);
    if (!body) return;

    setSavingCode(code);
    setRowError(null);
    try {
      await api.updateAccountType(code, body);
      await queryClient.invalidateQueries({ queryKey: ACCOUNT_TYPES_QUERY_KEY });
      if (accountTypeUpdateChangesRights(body)) {
        showBanner(t('settings.accountTypes.reloginHint'));
      }
    } catch (err) {
      if (axios.isAxiosError(err) && err.response?.status === 403) {
        setRowError(t('settings.accountTypes.forbidden'));
      } else {
        setRowError(getApiErrorMessage(err, t));
      }
    } finally {
      setSavingCode(null);
    }
  };

  const handleCreate = async () => {
    const codeProblem = accountTypeCodeError(createCode);
    const nameProblem = accountTypeNameError(createName);
    if (codeProblem === 'required') {
      setCreateFieldError(t('settings.accountTypes.codeInvalid'));
      return;
    }
    if (codeProblem) {
      setCreateFieldError(t('settings.accountTypes.codeInvalid'));
      return;
    }
    if (nameProblem === 'tooLong') {
      setCreateFieldError(t('settings.accountTypes.nameTooLong'));
      return;
    }
    if (nameProblem) {
      setCreateFieldError(t('settings.accountTypes.nameRequired'));
      return;
    }
    const body = buildCreateAccountTypeRequest(createCode, createName, createRights);
    if (!body) return;

    setCreating(true);
    setCreateFieldError(null);
    setCreateError(null);
    try {
      await api.createAccountType(body);
      await queryClient.invalidateQueries({ queryKey: ACCOUNT_TYPES_QUERY_KEY });
      setCreateOpen(false);
      setCreateCode('');
      setCreateName('');
      setCreateRights(blankAccountTypeRights());
      showBanner(t('settings.accountTypes.created'));
    } catch (err) {
      setCreateError(
        axios.isAxiosError(err) && err.response?.status === 403
          ? t('settings.accountTypes.forbidden')
          : getApiErrorMessage(err, t),
      );
    } finally {
      setCreating(false);
    }
  };

  const confirmDelete = async () => {
    if (!pendingDelete) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await api.deleteAccountType(pendingDelete.code);
      await queryClient.invalidateQueries({ queryKey: ACCOUNT_TYPES_QUERY_KEY });
      setPendingDelete(null);
    } catch (err) {
      setDeleteError(
        axios.isAxiosError(err) && err.response?.status === 403
          ? t('settings.accountTypes.forbidden')
          : getApiErrorMessage(err, t),
      );
    } finally {
      setDeleting(false);
    }
  };

  if (isLoading && data === undefined) {
    return (
      <div className="flex items-center justify-center h-48">
        <div className="h-8 w-8 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (isError) {
    return (
      <Card className="rounded-2xl border-gray-200/80 dark:border-gray-800/80 shadow-sm overflow-hidden">
        <p className="text-center text-gray-500 dark:text-gray-400 py-8">{t('settings.accountTypes.errorLoad')}</p>
      </Card>
    );
  }

  const deleteLabel = pendingDelete?.name?.trim() || pendingDelete?.code || '';

  return (
    <>
      <AccountTypesEditor
        rows={rows}
        originals={data ?? []}
        canEdit={canEdit}
        savingCode={savingCode}
        rowError={rowError}
        banner={banner}
        createOpen={createOpen}
        createCode={createCode}
        createName={createName}
        createRights={createRights}
        createFieldError={createFieldError}
        createError={createError}
        creating={creating}
        onUpdateRow={updateRow}
        onSaveRow={(code) => {
          void handleSaveRow(code);
        }}
        onDeleteRow={(code) => {
          const row = rows.find((item) => item.code === code) ?? null;
          setDeleteError(null);
          setPendingDelete(row);
        }}
        onOpenCreate={() => {
          setCreateFieldError(null);
          setCreateError(null);
          setCreateOpen(true);
        }}
        onCancelCreate={() => setCreateOpen(false)}
        onCreateCode={(value) => {
          setCreateCode(value);
          setCreateFieldError(null);
        }}
        onCreateName={(value) => {
          setCreateName(value);
          setCreateFieldError(null);
        }}
        onCreateRight={(field, value) => {
          setCreateRights((prev) => ({ ...prev, [field]: value }));
        }}
        onSubmitCreate={() => {
          void handleCreate();
        }}
      />
      <ConfirmDialog
        isOpen={pendingDelete !== null}
        onClose={() => {
          if (deleting) return;
          setPendingDelete(null);
          setDeleteError(null);
        }}
        onConfirm={() => {
          void confirmDelete();
        }}
        title={t('common.confirm')}
        message={t('settings.accountTypes.deleteConfirm', { name: deleteLabel })}
        confirmLabel={t('common.delete')}
        confirmVariant="danger"
        isLoading={deleting}
        error={deleteError}
      />
    </>
  );
}
