import { Link } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { Badge, Button, Card } from '@/components/common';
import {
  completeItemHref,
  isReadyToCirculate,
  missingCirculationFields,
  type CirculationField,
} from '@/utils/circulationReadiness';

export interface ReceivedCopy {
  itemId: string;
  biblioId: string;
  title?: string | null;
  barcode?: string | null;
  sourceId?: string | null;
  sourceName?: string | null;
  price?: string | null;
  borrowable: boolean;
}

interface ReceivedCopiesPanelProps {
  copies: ReceivedCopy[];
  onDismiss: () => void;
}

function fieldLabel(t: (key: string) => string, field: CirculationField): string {
  if (field === 'barcode') return t('items.missingBarcode');
  if (field === 'site') return t('items.missingSite');
  return t('items.missingPrice');
}

export default function ReceivedCopiesPanel({ copies, onDismiss }: ReceivedCopiesPanelProps) {
  const { t } = useTranslation();
  if (copies.length === 0) return null;

  return (
    <Card className="space-y-3 p-4">
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {t('acquisitions.receive.resultTitle')}
          </h2>
          <p className="mt-1 text-sm text-gray-500">{t('acquisitions.receive.resultHelp')}</p>
        </div>
        <Button type="button" variant="secondary" size="sm" onClick={onDismiss}>
          {t('acquisitions.receive.dismissResult')}
        </Button>
      </div>
      <ul className="space-y-2">
        {copies.map((copy) => {
          const ready = isReadyToCirculate(copy) && copy.borrowable;
          const missing = missingCirculationFields(copy);
          const href = copy.biblioId
            ? ready
              ? `/biblios/${copy.biblioId}`
              : completeItemHref(copy.biblioId, copy.itemId)
            : null;
          return (
            <li
              key={copy.itemId}
              className="flex flex-col gap-2 rounded-lg border border-gray-200 p-3 dark:border-gray-800 sm:flex-row sm:items-center sm:justify-between"
            >
              <div className="min-w-0 space-y-1">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="font-medium text-gray-900 dark:text-white">
                    {copy.barcode?.trim() || t('items.noBarcode')}
                  </p>
                  {ready ? (
                    <Badge variant="success">{t('items.borrowableYes')}</Badge>
                  ) : (
                    <Badge variant="warning">{t('items.incompleteNotCirculable')}</Badge>
                  )}
                </div>
                <p className="text-sm text-gray-500">
                  {copy.title?.trim() || copy.biblioId || copy.itemId}
                  {copy.sourceName ? ` · ${copy.sourceName}` : ''}
                  {copy.price ? ` · ${copy.price}` : ''}
                </p>
                {!ready && missing.length > 0 ? (
                  <p className="text-xs text-amber-700 dark:text-amber-400">
                    {t('items.missingCirculationFields', {
                      fields: missing.map((field) => fieldLabel(t, field)).join(', '),
                    })}
                  </p>
                ) : null}
              </div>
              {href ? (
                <Link
                  to={href}
                  className="shrink-0 text-sm font-medium text-amber-700 hover:underline dark:text-amber-400"
                >
                  {ready ? t('acquisitions.receive.openCopy') : t('acquisitions.receive.completeCopy')}
                </Link>
              ) : null}
            </li>
          );
        })}
      </ul>
    </Card>
  );
}
