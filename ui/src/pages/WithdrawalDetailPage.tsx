import { ArrowLeft } from 'lucide-react';
import { useNavigate, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Card, CardHeader } from '@/components/common';
import WithdrawalRecord from '@/components/items/WithdrawalRecord';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { navigateBack } from '@/utils/navigateBack';

export default function WithdrawalDetailPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { id } = useParams<{ id: string }>();
  const query = useQuery({
    queryKey: ['item-withdrawal-by-id', id],
    queryFn: () => api.getWithdrawal(id!),
    enabled: !!id,
  });

  return (
    <div className="space-y-4 min-w-0">
      <button
        type="button"
        onClick={() => navigateBack(navigate, '/weeding')}
        className="inline-flex items-center gap-1.5 rounded-lg p-2 text-sm text-amber-800 hover:bg-amber-50 dark:text-amber-300 dark:hover:bg-amber-950/40"
        aria-label={t('common.back')}
      >
        <ArrowLeft className="h-4 w-4" aria-hidden />
        {t('weeding.backToList')}
      </button>
      <Card>
        <CardHeader title={t('weeding.detailTitle')} />
        {query.isLoading ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('common.loading')}</p>
        ) : null}
        {query.isError ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {getApiErrorMessage(query.error, t)}
          </p>
        ) : null}
        {query.data ? <WithdrawalRecord withdrawal={query.data} onChanged={() => void query.refetch()} /> : null}
      </Card>
    </div>
  );
}
