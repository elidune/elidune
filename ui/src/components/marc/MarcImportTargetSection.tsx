import { Plus } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button, Input } from '@/components/common';
import type { DepositBatch, Site } from '@/types';
import type { MarcImportTargetVariant } from '@/utils/marcImportTarget';
import { formControlClass, formLabelClass } from '@/utils/formControl';

type CatalogProps = {
  variant: Extract<MarcImportTargetVariant, { kind: 'catalog' }>;
  sites: Site[];
  depositBatches: DepositBatch[];
  depositBatchesError: boolean;
  selectedDepositBatch: DepositBatch | null;
  showAddSource: boolean;
  newSourceName: string;
  addSourceLoading: boolean;
  sitesError: string;
  onStockMode: (mode: 'own' | 'deposit') => void;
  onSourceId: (id: string | null) => void;
  onDepositBatchId: (id: string) => void;
  onToggleAddSource: () => void;
  onNewSourceName: (value: string) => void;
  onAddSource: (event: React.FormEvent) => void;
  onCancelAddSource: () => void;
};

type DepositBatchProps = {
  variant: Extract<MarcImportTargetVariant, { kind: 'depositBatch' }>;
  sites: Site[];
  onSiteId: (id: string | null) => void;
};

type MarcImportTargetSectionProps =
  | ({ mode: 'catalog' } & CatalogProps)
  | ({ mode: 'depositBatch' } & DepositBatchProps);

function siteLabel(site: Site, t: (key: string) => string): string {
  const name = site.name || site.key || site.id;
  return site.default ? `${name} (${t('importMarc.default')})` : name;
}

export default function MarcImportTargetSection(props: MarcImportTargetSectionProps) {
  const { t } = useTranslation();

  if (props.mode === 'depositBatch') {
    const { variant, sites, onSiteId } = props;
    if (sites.length <= 1) {
      const only = sites[0];
      return (
        <p className="text-sm text-gray-600 dark:text-gray-300">
          {t('deposits.site')}: {only ? siteLabel(only, t) : variant.batchSiteId}
        </p>
      );
    }
    return (
      <div className="flex flex-wrap items-center gap-2">
        <label className={formLabelClass({ marginBottom: false, inline: true, className: 'shrink-0' })} htmlFor="deposit-marc-site">
          {t('deposits.import.targetSite')}
        </label>
        <select
          id="deposit-marc-site"
          value={variant.selectedSiteId ?? ''}
          onChange={(event) => onSiteId(event.target.value || null)}
          className={formControlClass({ className: 'flex-1 min-w-[200px]' })}
        >
          <option value="">{t('common.select')}</option>
          {sites.map((site) => (
            <option key={site.id} value={String(site.id)}>
              {siteLabel(site, t)}
            </option>
          ))}
        </select>
        <p className="w-full text-xs text-gray-500 dark:text-gray-400">{t('deposits.import.targetSiteHint')}</p>
      </div>
    );
  }

  const {
    variant,
    sites,
    depositBatches,
    depositBatchesError,
    selectedDepositBatch,
    showAddSource,
    newSourceName,
    addSourceLoading,
    sitesError,
    onStockMode,
    onSourceId,
    onDepositBatchId,
    onToggleAddSource,
    onNewSourceName,
    onAddSource,
    onCancelAddSource,
  } = props;

  return (
    <div className="space-y-3">
      <fieldset className="flex flex-wrap gap-4">
        <legend className={formLabelClass({ marginBottom: false })}>{t('deposits.import.mode')}</legend>
        <label className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200">
          <input
            type="radio"
            name="stock-mode"
            checked={variant.stockMode === 'own'}
            onChange={() => onStockMode('own')}
          />
          {t('deposits.import.ownStock')}
        </label>
        <label className="flex items-center gap-2 text-sm text-gray-800 dark:text-gray-200">
          <input
            type="radio"
            name="stock-mode"
            checked={variant.stockMode === 'deposit'}
            onChange={() => onStockMode('deposit')}
          />
          {t('deposits.import.deposit')}
        </label>
      </fieldset>
      {variant.stockMode === 'deposit' ? (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <label className={formLabelClass({ marginBottom: false, inline: true, className: 'shrink-0' })} htmlFor="deposit-batch-select">
              {t('deposits.import.batch')}
            </label>
            <select
              id="deposit-batch-select"
              value={variant.selectedDepositBatchId}
              onChange={(event) => onDepositBatchId(event.target.value)}
              className={formControlClass({ className: 'flex-1 min-w-[200px]' })}
            >
              <option value="">{t('deposits.import.chooseBatch')}</option>
              {depositBatches.map((batch) => (
                <option key={batch.id} value={batch.id}>
                  {batch.reference}
                </option>
              ))}
            </select>
          </div>
          {depositBatchesError ? (
            <p className="text-sm text-red-600 dark:text-red-400">{t('deposits.import.batchesError')}</p>
          ) : null}
          <p className="text-sm text-gray-600 dark:text-gray-300">
            {t('deposits.site')}:{' '}
            {selectedDepositBatch
              ? sites.find((site) => String(site.id) === selectedDepositBatch.siteId)?.name
                || selectedDepositBatch.siteId
              : '—'}
          </p>
        </div>
      ) : sites.length <= 1 ? (
        <p className="text-sm text-gray-600 dark:text-gray-300">
          {t('importMarc.site')}: {sites[0] ? siteLabel(sites[0], t) : '—'}
        </p>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <label className={formLabelClass({ marginBottom: false, inline: true, className: 'shrink-0' })} htmlFor="marc-import-site">
            {t('importMarc.site')} :
          </label>
          <select
            id="marc-import-site"
            value={variant.selectedSiteId ?? ''}
            onChange={(e) => onSourceId(e.target.value || null)}
            className={formControlClass({ className: 'flex-1 min-w-[200px]' })}
          >
            <option value="">{t('importMarc.noSite')}</option>
            {sites.map((site) => (
              <option key={site.id} value={site.id}>
                {siteLabel(site, t)}
              </option>
            ))}
          </select>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={onToggleAddSource}
            leftIcon={<Plus className="h-4 w-4" />}
          >
            {t('importMarc.addSite')}
          </Button>
        </div>
      )}
      {variant.stockMode === 'own' && showAddSource ? (
        <form onSubmit={onAddSource} className="mt-3 flex flex-wrap items-end gap-2">
          <Input
            value={newSourceName}
            onChange={(e) => onNewSourceName(e.target.value)}
            placeholder={t('importMarc.newSiteName')}
            className="flex-1 min-w-[180px]"
            autoFocus
          />
          <Button type="submit" size="sm" isLoading={addSourceLoading} disabled={!newSourceName.trim()}>
            {t('common.add')}
          </Button>
          <Button type="button" size="sm" variant="ghost" onClick={onCancelAddSource}>
            {t('common.cancel')}
          </Button>
        </form>
      ) : null}
      {sitesError ? <p className="mt-1 text-sm text-red-600 dark:text-red-400">{sitesError}</p> : null}
    </div>
  );
}

