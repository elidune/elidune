export type ItemQuickCreateValues = {
  barcode: string;
  callNumber: string;
  siteId: string;
  volumeDesignation: string;
  borrowable: 'true' | 'false';
};

export const emptyItemQuickCreateValues = (): ItemQuickCreateValues => ({
  barcode: '',
  callNumber: '',
  siteId: '',
  volumeDesignation: '',
  borrowable: 'true',
});
