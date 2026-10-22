import { useState } from 'react';
import { useNavigate, Navigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/common';
import { UserEditorForm } from '@/components/users';
import { useAuth } from '@/contexts/AuthContext';
import { canManageUsers, type User } from '@/types';
import api from '@/services/api';
import { navigateBack } from '@/utils/navigateBack';
import { useTranslation } from 'react-i18next';
import { usePublicTypesQuery } from '@/hooks/usePublicTypesQuery';
import { useAccountTypesQuery } from '@/hooks/useAccountTypesQuery';
import { patronId } from '@/utils/patronLink';

export default function UserCreatePage() {
  const { user } = useAuth();
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { data: publicTypes = [] } = usePublicTypesQuery();
  const accountTypesQuery = useAccountTypesQuery();
  const accountTypes = accountTypesQuery.data ?? [];
  const accountTypesError = accountTypesQuery.isError ? t('settings.accountTypes.errorLoad') : null;

  const [isLoading, setIsLoading] = useState(false);

  if (!canManageUsers(user, api.getToken())) {
    return <Navigate to="/users" replace />;
  }

  const goAfterCreate = (created?: User) => {
    const id = patronId(created?.id);
    navigate(id ? `/users/${id}` : '/users');
  };

  return (
    <div className="pb-12">
      <div className="sticky top-0 z-10 -mx-4 -mt-4 lg:-mx-6 lg:-mt-6 px-4 pt-4 pb-4 lg:px-6 lg:pt-6 mb-6 bg-gray-50 dark:bg-gray-950 border-b border-gray-200/80 dark:border-gray-800/80">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex items-start sm:items-center gap-3 min-w-0">
            <button
              type="button"
              onClick={() => navigateBack(navigate, '/users')}
              className="p-2 rounded-lg hover:bg-gray-200 dark:hover:bg-gray-800 shrink-0"
              aria-label={t('common.back')}
            >
              <ArrowLeft className="h-5 w-5" />
            </button>
            <div className="min-w-0">
              <h1 className="text-xl sm:text-2xl font-bold text-gray-900 dark:text-white truncate">
                {t('users.newUser')}
              </h1>
            </div>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2 shrink-0">
            <Button variant="secondary" type="button" onClick={() => navigateBack(navigate, '/users')}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" form="create-user-form" isLoading={isLoading}>
              {t('common.create')}
            </Button>
          </div>
        </div>
      </div>

      <div className="rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 p-5 sm:p-8 lg:p-10 shadow-sm">
        <UserEditorForm
          mode="create"
          formId="create-user-form"
          publicTypes={publicTypes}
          accountTypes={accountTypes}
          accountTypesError={accountTypesError}
          onLoadingChange={setIsLoading}
          onSuccess={goAfterCreate}
        />
      </div>
    </div>
  );
}
