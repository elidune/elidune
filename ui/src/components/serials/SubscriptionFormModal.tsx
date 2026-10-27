import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQuery } from '@tanstack/react-query';
import { Button, Modal } from '@/components/common';
import SubscriptionFormFields from '@/components/serials/SubscriptionFormFields';
import { useFundsPickerQuery } from '@/hooks/acquisitions/useFundsQuery';
import { useActiveVendorsQuery } from '@/hooks/acquisitions/useVendorsQuery';
import api from '@/services/api';
import type { SerialSubscription, SerialSubscriptionWrite } from '@/types';
import { getApiErrorMessage } from '@/utils/apiError';
import {
  buildSubscriptionWrite,
  emptySubscriptionForm,
  isEmptyHorizonError,
  predictionBody,
  subscriptionToForm,
  type SubscriptionFormValues,
} from '@/utils/serialSubscriptionDisplay';

interface SubscriptionFormModalProps {
  subscription: SerialSubscription | null;
  isOpen: boolean;
  isSaving: boolean;
  error: unknown;
  onClose: () => void;
  onSubmit: (body: SerialSubscriptionWrite) => void;
}

export default function SubscriptionFormModal({
  subscription,
  isOpen,
  isSaving,
  error,
  onClose,
  onSubmit,
}: SubscriptionFormModalProps) {
  const { t } = useTranslation();
  const [values, setValues] = useState<SubscriptionFormValues>(() =>
    subscription ? subscriptionToForm(subscription) : emptySubscriptionForm(),
  );
  const [errors, setErrors] = useState<Record<string, 'required' | 'invalidAmount' | 'invalidNumber'>>({});
  const [biblioQuery, setBiblioQuery] = useState('');
  const [debouncedBiblio, setDebouncedBiblio] = useState('');

  const frequenciesQuery = useQuery({
    queryKey: ['serial-frequencies'],
    queryFn: () => api.listSerialFrequencies(),
    enabled: isOpen,
    staleTime: 60_000,
  });
  const patternsQuery = useQuery({
    queryKey: ['serial-patterns'],
    queryFn: () => api.listSerialPatterns(),
    enabled: isOpen,
    staleTime: 60_000,
  });
  const sitesQuery = useQuery({
    queryKey: ['sites', 'active'],
    queryFn: () => api.getSites(false),
    enabled: isOpen,
  });
  const vendorsQuery = useActiveVendorsQuery(isOpen);
  const fundsQuery = useFundsPickerQuery(isOpen);
  const biblioSearch = useQuery({
    queryKey: ['serial-periodicals', debouncedBiblio],
    queryFn: () => api.getBiblios({ title: debouncedBiblio, mediaType: 'periodic', perPage: 8, page: 1 }),
    enabled: isOpen && debouncedBiblio.trim().length >= 2 && !values.biblioId,
  });

  const request = useMemo(() => predictionBody(values), [values]);
  const [debouncedRequest, setDebouncedRequest] = useState(request);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedRequest(request), 400);
    return () => window.clearTimeout(timer);
  }, [request]);
  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedBiblio(biblioQuery), 300);
    return () => window.clearTimeout(timer);
  }, [biblioQuery]);

  const previewQuery = useQuery({
    queryKey: ['serial-prediction', debouncedRequest],
    queryFn: () => api.predictSerialIssues(debouncedRequest!),
    enabled: isOpen && debouncedRequest != null,
    retry: false,
  });

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    const built = buildSubscriptionWrite(values, subscription);
    if (!built.ok) {
      setErrors(built.errors);
      return;
    }
    setErrors({});
    onSubmit(built.body);
  };

  const submitMessage = error ? getApiErrorMessage(error, t) : null;
  const submitEmpty = Boolean(error) && isEmptyHorizonError(error);
  const previewError = previewQuery.isError
    ? getApiErrorMessage(previewQuery.error, t)
    : submitEmpty
      ? submitMessage
      : null;
  const emptyHorizon =
    (previewQuery.isError && isEmptyHorizonError(previewQuery.error)) || submitEmpty;

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={subscription ? t('serials.form.editTitle') : t('serials.form.createTitle')}
      size="xl"
      footer={
        <div className="flex flex-wrap justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isSaving}>
            {t('common.cancel')}
          </Button>
          <Button type="submit" form="serial-subscription-form" isLoading={isSaving}>
            {t('common.save')}
          </Button>
        </div>
      }
    >
      <form id="serial-subscription-form" className="space-y-3" onSubmit={handleSubmit}>
        <SubscriptionFormFields
          values={values}
          onChange={(patch) => setValues((current) => ({ ...current, ...patch }))}
          errors={errors}
          frequencies={frequenciesQuery.data?.frequencies ?? []}
          patterns={patternsQuery.data?.patterns ?? []}
          sites={sitesQuery.data ?? []}
          vendors={(vendorsQuery.data?.vendors ?? []).filter((vendor) => vendor.active && !vendor.archivedAt)}
          funds={fundsQuery.data?.funds ?? []}
          biblioQuery={biblioQuery}
          onBiblioQuery={setBiblioQuery}
          biblioResults={biblioSearch.data?.items ?? []}
          biblioSearching={biblioSearch.isFetching}
          onSelectBiblio={(id, title) => {
            setValues((current) => ({ ...current, biblioId: id, biblioTitle: title }));
            setBiblioQuery('');
          }}
          predictionLocked={(subscription?.issueCount ?? 0) > 0}
          preview={previewQuery.data?.issues ?? []}
          previewLoading={previewQuery.isFetching}
          previewError={previewError}
          emptyHorizon={emptyHorizon}
          showStatus={subscription != null}
        />
        {submitMessage && !submitEmpty ? (
          <p role="alert" className="text-sm text-red-600 dark:text-red-400">
            {submitMessage}
          </p>
        ) : null}
      </form>
    </Modal>
  );
}
