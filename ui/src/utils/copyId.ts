import api from '@/services/api';
import { itemIdFromCopies } from '@/utils/lostFound';

/** Copy id for a scanned barcode. Null when the notice cannot be read. */
export async function copyIdForBarcode(barcode: string): Promise<string | null> {
  try {
    const notice = await api.getItemByBarcode(barcode);
    return itemIdFromCopies(notice.items, barcode);
  } catch {
    return null;
  }
}
