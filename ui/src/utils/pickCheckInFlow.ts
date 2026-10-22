import type { TFunction } from 'i18next';
import { canReadUsers } from '@/types';
import api from '@/services/api';
import { getApiErrorMessage } from '@/utils/apiError';
import { heldSetAsideServerMessage } from '@/utils/heldSetAside';
import { returnDeskNotice, returnNoticeMessage, returnScanFeedback } from '@/utils/returnNotice';
import { sourceLabel } from '@/utils/transitDisplay';
import { interpretPickCheckIn, type PickCheckInNotice } from '@/utils/toPickDesk';

export interface PickCheckInBind {
  holdId: string;
  itemId: string;
  barcode?: string | null;
  title?: string | null;
}

export async function resolvePickNotice(notice: PickCheckInNotice): Promise<PickCheckInNotice> {
  let patronName = notice.patronName;
  let siteName = notice.siteName;
  if (notice.kind === 'ready' && !patronName && notice.userId && canReadUsers(null, api.getToken())) {
    try {
      const user = await api.getUser(notice.userId);
      const joined = [user.firstname, user.lastname].filter(Boolean).join(' ').trim();
      patronName = joined || null;
    } catch {
      patronName = null;
    }
  }
  if (notice.kind === 'transit' && !siteName && notice.siteId) {
    try {
      const sites = await api.getSites(false);
      siteName = sourceLabel(sites, notice.siteId);
    } catch {
      siteName = null;
    }
  }
  return { ...notice, patronName, siteName };
}

export function pickCheckInBannerMessage(notice: PickCheckInNotice, t: TFunction): string {
  return notice.kind === 'ready'
    ? t('loans.pickReady', { patron: notice.patronName?.trim() || t('loans.pickPatronUnknown') })
    : t('loans.pickTransit', { site: notice.siteName?.trim() || t('loans.pickSiteUnknown') });
}

export type PickCheckInFlowResult =
  | { kind: 'pick'; notice: PickCheckInNotice }
  | { kind: 'deskNotice'; message: string }
  | { kind: 'unexpected'; message: string };

/**
 * Desk check-in for a pulled hold copy (`POST /loans/items/{barcode}/return`).
 * When `bind` is set, the server validates hold and item ids (set-aside).
 */
export async function runPickCheckInFlow(
  barcode: string,
  bind: PickCheckInBind | null | undefined,
  t: TFunction,
): Promise<PickCheckInFlowResult> {
  const trimmed = barcode.trim();
  if (!trimmed) {
    return { kind: 'unexpected', message: t('loans.barcodeRequired') };
  }

  const result = bind
    ? await api.returnLoanByBarcode(trimmed, { holdId: bind.holdId, itemId: bind.itemId })
    : await api.returnLoanByBarcode(trimmed);

  const pick = interpretPickCheckIn(result);
  const feedback = returnScanFeedback(result, { pick: Boolean(pick), hasLoan: Boolean(result.loan) });

  if (feedback.toast === 'none' && feedback.notice) {
    return { kind: 'deskNotice', message: returnNoticeMessage(feedback.notice, t) };
  }

  if (pick) {
    const notice = await resolvePickNotice(pick);
    return { kind: 'pick', notice };
  }

  if (result.loan) {
    return { kind: 'unexpected', message: t('holds.checkInWasLoanReturn') };
  }

  const desk = returnDeskNotice(result);
  if (desk?.level === 'informational') {
    return { kind: 'unexpected', message: t('holds.checkInNotToPick') };
  }

  return { kind: 'unexpected', message: t('holds.checkInNotToPick') };
}

export function pickCheckInErrorMessage(error: unknown, t: TFunction): string {
  return heldSetAsideServerMessage(error, t) || getApiErrorMessage(error, t) || t('holds.checkInError');
}
