import type { Site } from '@/types';

/** How the MARC import chooses catalog site vs deposit batch. */
export type MarcImportTargetVariant =
  | { kind: 'catalog'; stockMode: 'own' | 'deposit'; selectedSiteId: string | null; selectedDepositBatchId: string }
  | {
      kind: 'depositBatch';
      depositBatchId: string;
      batchSiteId: string;
      /** Confirmed site when the library has several sites; must match the batch site. */
      selectedSiteId: string | null;
      sites: Site[];
    };

export function marcImportTargetReady(variant: MarcImportTargetVariant): boolean {
  if (variant.kind === 'catalog') {
    return variant.stockMode === 'deposit'
      ? Boolean(variant.selectedDepositBatchId)
      : Boolean(variant.selectedSiteId);
  }
  if (variant.sites.length <= 1) return true;
  return variant.selectedSiteId === variant.batchSiteId;
}

export function marcImportApiSiteId(variant: MarcImportTargetVariant): string | number | null {
  if (variant.kind === 'catalog') {
    return variant.stockMode === 'deposit' ? null : variant.selectedSiteId;
  }
  if (variant.sites.length <= 1) return null;
  return variant.selectedSiteId;
}

export function marcImportDepositBatchId(variant: MarcImportTargetVariant): string | null {
  if (variant.kind === 'catalog') {
    return variant.stockMode === 'deposit' ? variant.selectedDepositBatchId || null : null;
  }
  return variant.depositBatchId;
}

export function marcImportAllowsDuplicateIsbn(variant: MarcImportTargetVariant): boolean {
  return variant.kind === 'catalog' && variant.stockMode === 'own';
}

export function marcImportTargetRequiredMessage(
  variant: MarcImportTargetVariant,
  t: (key: string) => string,
): string {
  if (variant.kind === 'depositBatch') {
    return variant.sites.length > 1 ? t('deposits.import.targetSiteRequired') : '';
  }
  return variant.stockMode === 'deposit' ? t('deposits.import.batchRequired') : t('importMarc.siteRequired');
}
