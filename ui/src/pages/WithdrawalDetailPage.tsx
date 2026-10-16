import { Link, useParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Card, CardHeader } from '@/components/common';
import WithdrawalRecord from '@/components/items/WithdrawalRecord';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';

export default function WithdrawalDetailPage() {
  const { t } = useTranslation();
  const { id } = useParams<{ id: string }>();
  const query = useQuery({
    queryKey: ['item-withdrawal-by-id', id],
    queryFn: () => api.getWithdrawal(id!),
    enabled: !!id,
  });

  return (
    <div className="space-y-4 min-w-0">
      <p className="text-sm">
        <Link className="text-amber-700 hover:underline dark:text-amber-400" to="/weeding">
          {t('weeding.backToList')}
        </Link>
      </p>
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
