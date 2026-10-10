import { useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import axios from 'axios';
import { ArrowLeft, MailX } from 'lucide-react';
import { Button, Card } from '@/components/common';
import { useLibrary } from '@/contexts/LibraryContext';
import api from '@/services/api';

type UnsubscribeState = 'ready' | 'submitting' | 'success' | 'invalid' | 'error';

export default function UnsubscribeEventsPage() {
  const { t } = useTranslation();
  const { libraryName } = useLibrary();
  const [searchParams] = useSearchParams();
  const token = (searchParams.get('token') ?? '').trim();
  const [state, setState] = useState<UnsubscribeState>(token ? 'ready' : 'invalid');

  const confirm = async () => {
    if (!token) {
      setState('invalid');
      return;
    }
    setState('submitting');
    try {
      const status = await api.unsubscribeFromEvents(token);
      setState(status === 204 ? 'success' : 'error');
    } catch (error) {
      if (axios.isAxiosError(error) && error.response?.status === 400) {
        setState('invalid');
      } else {
        setState('error');
      }
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-amber-50 via-white to-gray-50 dark:from-gray-950 dark:via-gray-900 dark:to-gray-800 p-4">
      <div className="absolute inset-0 overflow-hidden pointer-events-none">
        <div className="absolute -top-40 -right-40 w-80 h-80 bg-amber-200 dark:bg-amber-900/30 rounded-full blur-3xl opacity-50" />
        <div className="absolute -bottom-40 -left-40 w-80 h-80 bg-gray-200 dark:bg-gray-800/30 rounded-full blur-3xl opacity-50" />
      </div>

      <Card className="w-full max-w-md relative">
        <div className="mb-6">
          <Link
            to="/"
            className="inline-flex items-center gap-1.5 text-sm font-medium text-amber-600 dark:text-amber-400 hover:underline"
          >
            <ArrowLeft className="h-4 w-4" />
            {t('eventsConsent.unsubscribe.backHome')}
          </Link>
        </div>

        <div className="text-center mb-6">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-amber-100 dark:bg-amber-900/50 mb-4">
            <MailX className="h-8 w-8 text-amber-600 dark:text-amber-400" aria-hidden />
          </div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
            {t('eventsConsent.unsubscribe.title')}
          </h1>
          {libraryName ? (
            <p className="text-gray-400 dark:text-gray-500 text-xs mt-2">{libraryName}</p>
          ) : null}
        </div>

        {state === 'success' ? (
          <p className="text-sm text-gray-700 dark:text-gray-300 leading-relaxed text-center" role="status">
            {t('eventsConsent.unsubscribe.success')}
          </p>
        ) : state === 'invalid' ? (
          <p className="text-sm text-gray-700 dark:text-gray-300 leading-relaxed text-center" role="alert">
            {token ? t('eventsConsent.unsubscribe.invalid') : t('eventsConsent.unsubscribe.missing')}
          </p>
        ) : (
          <div className="space-y-4">
            <p className="text-sm text-gray-700 dark:text-gray-300 leading-relaxed">
              {t('eventsConsent.unsubscribe.explain')}
            </p>
            {state === 'error' ? (
              <p className="text-sm text-red-600 dark:text-red-400" role="alert">
                {t('eventsConsent.unsubscribe.error')}
              </p>
            ) : null}
            <Button
              type="button"
              className="w-full"
              size="lg"
              isLoading={state === 'submitting'}
              onClick={() => {
                void confirm();
              }}
            >
              {t('eventsConsent.unsubscribe.confirm')}
            </Button>
          </div>
        )}
      </Card>
    </div>
  );
}
