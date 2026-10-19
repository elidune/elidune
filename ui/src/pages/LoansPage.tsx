import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import {
  BookMarked,
  RotateCcw,
  Check,
  CreditCard,
  X,
  AlertTriangle,
  BookOpen,
  Bell,
  Send,
  FlaskConical,
  ChevronDown,
  ClipboardList,
} from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { Card, CardHeader, Button, Badge, Table, MessageModal, ConfirmDialog, ScrollableListRegion, ResponsiveRecordList, ListSkeleton, BarcodeScanField } from '@/components/common';
import ItemAvailabilityBadge from '@/components/items/ItemAvailabilityBadge';
import ActiveLoanCard from '@/components/loans/ActiveLoanCard';
import CirculationExceptionDialog from '@/components/loans/CirculationExceptionDialog';
import ClaimsReturnedQueue from '@/components/loans/ClaimsReturnedQueue';
import LoanExceptionActions from '@/components/loans/LoanExceptionActions';
import DeskScanNotice from '@/components/loans/DeskScanNotice';
import ReturnDeskNotice from '@/components/loans/ReturnDeskNotice';
import ForceCirculationDialog from '@/components/loans/ForceCirculationDialog';
import ToPickForceLoanDialog from '@/components/loans/ToPickForceLoanDialog';
import LostFoundAlertDialog from '@/components/loans/LostFoundAlertDialog';
import Pagination from '@/components/common/Pagination';
import api from '@/services/api';
import { getApiErrorCode, getApiErrorMessage, getApiErrorRawMessage, getHttpStatus } from '@/utils/apiError';
import {
  classifyLostFoundError,
  isLostReturn,
  itemIdFromCopies,
  itemIdFromLostReturn,
  lostFoundActionLabel,
  lostFoundErrorMessage,
  lostFoundPanelFee,
  lostFoundResolutionMessage,
  type LostFoundPanelAction,
} from '@/utils/lostFound';
import {
  CheckoutScanNoticeShown,
  checkoutAlertAfterSuccessfulReturn,
  checkoutScanStopsLoan,
  deskScanAlert,
  deskScanAlertAfterReturn,
  isCheckoutScanNoticeShown,
  otherBorrowerFromPerson,
  readerName,
  type DeskCopySnapshot,
  type DeskScanAlert,
} from '@/utils/deskScanAlert';
import { checkoutForceFlag, checkoutNoticeForConflict } from '@/utils/loanConflict';
import { renewedUntilMessage } from '@/utils/localeDate';
import { depositDueDateNotice, lenderNameFromCopies, returnToLenderNotice, withDepositDueNotice, withReturnToLenderNotice } from '@/utils/deposits';
import ReturnToLenderNotice from '@/components/items/ReturnToLenderNotice';
import DepositBadge from '@/components/items/DepositBadge';
import { patronId } from '@/utils/patronLink';
import { isWeedingServerMessage } from '@/utils/weeding';
import { sortLoansByStartDateAsc } from '@/utils/sortLoans';
import { isClaimedReturnedLoan } from '@/utils/circulationStatus';
import { deskBorrowErrorText, presentCheckoutRefusal } from '@/utils/deskCheckoutError';
import {
  CheckoutDeclined,
  interpretPickCheckIn,
  isCheckoutDeclined,
  type PickCheckInNotice,
} from '@/utils/toPickDesk';
import { overrideReasonRefusalMessage, trimmedOverrideReason } from '@/utils/circulationForce';
import { copyIdForBarcode } from '@/utils/copyId';
import {
  commitDismissHeldSetAside,
  commitHeldSetAside,
  heldForOtherHoldId,
  heldSetAsideServerMessage,
  heldSetAsideSuccessMessage,
} from '@/utils/heldSetAside';
import {
  returnNoticeMessage,
  returnNoticeOffersLostFound,
  returnScanFeedback,
  type ReturnDeskNotice as ReturnDeskNoticeModel,
} from '@/utils/returnNotice';
import { sourceLabel } from '@/utils/transitDisplay';
import { formatIsbnDisplay } from '@/utils/isbnDisplay';
import { LoanMediaTypeBadge } from '@/utils/mediaTypeIcon';
import { formControlClass, formLabelClass, formChoiceLabelClass } from '@/utils/formControl';
import { deferFromEffect } from '@/utils/deferFromEffect';
import { newIdempotencyKey } from '@/utils/idempotency';
import { nextReminderTierFromCount } from '@/utils/reminderPolicy';
import { useCirculationExceptionAction, type CirculationExceptionKind } from '@/hooks/loans/useCirculationExceptionAction';
import { useAuth } from '@/contexts/AuthContext';
import DeskNameSearch from '@/components/loans/DeskNameSearch';
import DeskPatronStatus from '@/components/loans/DeskPatronStatus';
import OverdueBorrowerSummary from '@/components/loans/OverdueBorrowerSummary';
import OverdueDueDate from '@/components/loans/OverdueDueDate';
import PatronLoansOutcome from '@/components/loans/PatronLoansOutcome';
import { OVERDUE_FACTS_CLASS } from '@/utils/overdueDueDate';
import ReturnBorrowerLine from '@/components/loans/ReturnBorrowerLine';
import { useToast } from '@/contexts/ToastContext';
import { canForceLoans, canReadUsers, isAdmin } from '@/types';
import type { User as UserType, Loan, UserShort, OverdueLoanInfo, ReminderReport, ItemCheckIn, LostFoundDisposition, LostReturnFields, PatronLoanStatus, LoanListItem } from '@/types';
import { isItemStaffView, isUserRecordLoan } from '@/utils/catalogView';
import RenewalRefusalNotice from '@/components/loans/RenewalRefusalNotice';
import { presentRenewalRefusal, type RenewalDeskView } from '@/utils/renewalDesk';
import { deskNameSearchVisible, deskPatronIdentity, patronFromCardScan, presentCardScanError } from '@/utils/deskPatron';
import { readerIdentifierText, type BorrowerRef } from '@/utils/readerIdentifier';
import {
  checkoutRecap,
  currentUserLoansQuery,
  deskBorrower,
  readPatronLoans,
} from '@/utils/loanListAccess';

type TabType = 'borrow' | 'return' | 'overdue' | 'claims';

const LOAN_TAB_IDS: TabType[] = ['borrow', 'return', 'overdue', 'claims'];

function isLoanTab(value: string | null): value is TabType {
  return value != null && LOAN_TAB_IDS.includes(value as TabType);
}

type LostFoundSummary = {
  title: string | null;
  barcode: string | null;
  patronName: string | null;
  foundAt: string;
  action: LostFoundPanelAction;
  feeText: string;
};

type ReturnView =
  | { kind: 'loan'; loan: Loan }
  | { kind: 'pick'; notice: PickCheckInNotice; deskNotice?: ReturnDeskNoticeModel | null }
  | { kind: 'item'; title: string | null; barcode: string | null; availability: string | null }
  | ({ kind: 'lostFound' } & LostFoundSummary)
  | { kind: 'deskNotice'; notice: ReturnDeskNoticeModel; putBack: boolean; barcode: string | null };

type CheckoutScanAlert = DeskScanAlert & { barcode: string; loanId?: string | null };
type DeskLoanOp = 'return' | 'renew' | CirculationExceptionKind;

type CheckoutRecap = {
  title: string;
  barcode: string;
  dueDate: string;
  patronName: string;
  dueDateCappedByDeposit?: boolean;
};

const BORROW_LOANS_PAGE_SIZE = 20;

function deskPatronLabel(identity: BorrowerRef | null, cardLabel: (number: string) => string): string {
  return readerIdentifierText(identity, cardLabel);
}

function maxReminderSentAt(loans: OverdueLoanInfo[]): string | null {
  let best: string | null = null;
  for (const l of loans) {
    const t = l.lastReminderSentAt;
    if (!t) continue;
    if (!best || t > best) best = t;
  }
  return best;
}

/** Calendar days since due date (0 if not past). */
function daysPastDue(expiryAt: string | null): number {
  if (!expiryAt) return 0;
  const due = new Date(expiryAt);
  const today = new Date();
  const d0 = Date.UTC(due.getFullYear(), due.getMonth(), due.getDate());
  const d1 = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate());
  return Math.max(0, Math.round((d1 - d0) / 86400000));
}

export default function LoansPage() {
  const { t, i18n } = useTranslation();
  const [searchParams] = useSearchParams();
  const { user: authUser } = useAuth();
  const canReadBorrowers = canReadUsers(authUser, api.getToken());
  const canForceLoan = canForceLoans(authUser, api.getToken());
  const { showToast, dismissPersistentErrors } = useToast();
  const userIsAdmin = isAdmin(authUser?.accountType);
  const tabFromUrl = searchParams.get('tab');
  const [activeTab, setActiveTab] = useState<TabType>(() =>
    isLoanTab(tabFromUrl) ? tabFromUrl : 'borrow',
  );
  const loansSearch = searchParams.toString();
  const [loansSearchSync, setLoansSearchSync] = useState(loansSearch);
  if (loansSearch !== loansSearchSync) {
    setLoansSearchSync(loansSearch);
    if (isLoanTab(tabFromUrl)) setActiveTab(tabFromUrl);
  }
  
  // Borrow section state
  const [selectedUser, setSelectedUser] = useState<UserType | null>(null);
  const [loans, setLoans] = useState<LoanListItem[]>([]);
  const [loansPage, setLoansPage] = useState(1);
  const [loansTotal, setLoansTotal] = useState(0);
  const [isLoadingLoans, setIsLoadingLoans] = useState(false);
  const [userSearchDraft, setUserSearchDraft] = useState('');
  const [userSearchResults, setUserSearchResults] = useState<UserShort[]>([]);
  const [isSearchingUsers, setIsSearchingUsers] = useState(false);
  const [isBorrowLoading, setIsBorrowLoading] = useState(false);
  const [barcodeInput, setBarcodeInput] = useState('');
  const barcodeInputRef = useRef<HTMLInputElement>(null);
  const userBarcodeInputRef = useRef<HTMLInputElement>(null);
  const [lastCheckout, setLastCheckout] = useState<CheckoutRecap | null>(null);
  const [borrowLoanAction, setBorrowLoanAction] = useState<{ loanId: string; op: DeskLoanOp } | null>(
    null,
  );
  const [userSearchError, setUserSearchError] = useState<string | null>(null);
  const [userSelectError, setUserSelectError] = useState<string | null>(null);
  const [loansLoadError, setLoansLoadError] = useState<string | null>(null);
  const [loansAccessDenied, setLoansAccessDenied] = useState(false);
  const [userSearchRetry, setUserSearchRetry] = useState(0);
  const [loansReloadToken, setLoansReloadToken] = useState(0);
  const [isPatronLookupBusy, setIsPatronLookupBusy] = useState(false);
  const [deskLoanStatus, setDeskLoanStatus] = useState<PatronLoanStatus | null>(null);
  const [patronIdentity, setPatronIdentity] = useState<BorrowerRef | null>(null);
  const [cardScanError, setCardScanError] = useState<string | null>(null);
  const [renewalPrompt, setRenewalPrompt] = useState<RenewalDeskView | null>(null);
  const renewalRetryRef = useRef<((overrideReason?: string) => void) | null>(null);
  const renewalSetAsideRef = useRef<{
    barcode: string;
    holdId: string | null;
    itemId: string | null;
  } | null>(null);
  const [renewalAsideLoading, setRenewalAsideLoading] = useState(false);
  const patronLookupInFlightRef = useRef(false);
  const borrowLoanActionInFlightRef = useRef(false);

  // Return section state
  const [returnBarcodeInput, setReturnBarcodeInput] = useState('');
  const returnBarcodeInputRef = useRef<HTMLInputElement>(null);
  const [returnResult, setReturnResult] = useState<ReturnView | null>(null);
  const [returnToLenderMessage, setReturnToLenderMessage] = useState<string | null>(null);
  const [isProcessingReturn, setIsProcessingReturn] = useState(false);
  const [returnError, setReturnError] = useState('');
  const [lostFound, setLostFound] = useState<(LostReturnFields & { itemId: string | null }) | null>(null);
  const [lostFoundError, setLostFoundError] = useState<string | null>(null);
  const [lostFoundPending, setLostFoundPending] = useState<LostFoundDisposition | null>(null);
  const [lostFoundLookup, setLostFoundLookup] = useState(false);
  const lostFoundOpenRef = useRef(false);
  const lostFoundLookupRef = useRef(false);
  const lostFoundFlightRef = useRef(false);
  useEffect(() => {
    lostFoundOpenRef.current = lostFound !== null;
  }, [lostFound]);
  const [renewError, setRenewError] = useState('');
  const [messageDialog, setMessageDialog] = useState<string | null>(null);

  const [overduePage, setOverduePage] = useState(1);
  const [overduePerPage, setOverduePerPage] = useState(50);
  const [overdueData, setOverdueData] = useState<{ loans: OverdueLoanInfo[]; total: number } | null>(null);
  const [overdueLoading, setOverdueLoading] = useState(false);
  const [overdueError, setOverdueError] = useState<string | null>(null);
  const [overdueLoanAction, setOverdueLoanAction] = useState<{ loanId: string; op: DeskLoanOp } | null>(
    null,
  );
  const overdueLoanActionInFlightRef = useRef(false);
  /** Keeps <details> open per user after renew/return refresh when they still have overdue loans */
  const [overdueOpenByUser, setOverdueOpenByUser] = useState<Record<string, boolean>>({});
  const [reminderReport, setReminderReport] = useState<ReminderReport | null>(null);
  const [reminderSending, setReminderSending] = useState(false);
  const [sendRemindersConfirmOpen, setSendRemindersConfirmOpen] = useState(false);
  const [forceBorrowDialog, setForceBorrowDialog] = useState<{
    message: string;
    barcode: string;
    reasonRequired: boolean;
    error: string | null;
  } | null>(null);
  const [forceBorrowLoading, setForceBorrowLoading] = useState(false);
  const [toPickAside, setToPickAside] = useState<{
    barcode: string;
    holdId: string | null;
    itemId: string | null;
    message: string;
    force: boolean;
    reasonRequired: boolean;
    error: string | null;
  } | null>(null);
  const [toPickAsideLoading, setToPickAsideLoading] = useState(false);
  const [checkoutScanAlert, setCheckoutScanAlert] = useState<CheckoutScanAlert | null>(null);
  const [returnScanAlert, setReturnScanAlert] = useState<DeskScanAlert | null>(null);

  /** Restore keyboard/scanner focus to the visible barcode field after dialogs close. */
  const focusScanInput = useCallback(() => {
    const apply = () => {
      if (lostFoundOpenRef.current) return;
      if (activeTab === 'return') {
        returnBarcodeInputRef.current?.focus();
        return;
      }
      if (activeTab === 'borrow') {
        if (selectedUser) {
          barcodeInputRef.current?.focus();
        } else {
          userBarcodeInputRef.current?.focus();
        }
      }
    };
    requestAnimationFrame(() => {
      apply();
      setTimeout(apply, 0);
      setTimeout(apply, 100);
    });
  }, [activeTab, selectedUser]);

  const selectedReaderIdRef = useRef<string | null>(null);
  const selectDeskReader = useCallback((user: UserType | null) => {
    const nextId = user?.id ?? null;
    if (selectedReaderIdRef.current !== nextId) {
      selectedReaderIdRef.current = nextId;
      dismissPersistentErrors();
    }
    setSelectedUser(user);
  }, [dismissPersistentErrors]);

  const resetBorrowFlow = useCallback(() => {
    selectDeskReader(null);
    setPatronIdentity(null);
    setDeskLoanStatus(null);
    setCardScanError(null);
    setRenewalPrompt(null);
    renewalRetryRef.current = null;
    renewalSetAsideRef.current = null;
    setLoans([]);
    setLoansTotal(0);
    setLoansPage(1);
    setLoansAccessDenied(false);
    setUserSearchDraft('');
    setUserSearchResults([]);
    setBarcodeInput('');
    setRenewError('');
    setLastCheckout(null);
    setUserSearchError(null);
    setUserSelectError(null);
    setLoansLoadError(null);
    if (userBarcodeInputRef.current) {
      userBarcodeInputRef.current.value = '';
    }
  }, [selectDeskReader]);

  const prevActiveTabRef = useRef<TabType | null>(null);

  useEffect(() => {
    const prev = prevActiveTabRef.current;
    if (activeTab === 'borrow' && prev !== null && prev !== 'borrow') {
      resetBorrowFlow();
      requestAnimationFrame(() => {
        userBarcodeInputRef.current?.focus();
        setTimeout(() => userBarcodeInputRef.current?.focus(), 0);
      });
    }
    if (activeTab === 'return' && prev !== null && prev !== 'return') {
      requestAnimationFrame(() => {
        if (lostFoundOpenRef.current) return;
        returnBarcodeInputRef.current?.focus();
        setTimeout(() => {
          if (!lostFoundOpenRef.current) returnBarcodeInputRef.current?.focus();
        }, 0);
        setTimeout(() => {
          if (!lostFoundOpenRef.current) returnBarcodeInputRef.current?.focus();
        }, 100);
      });
    }
    prevActiveTabRef.current = activeTab;
  }, [activeTab, resetBorrowFlow]);

  /** Keep return barcode field focused for scanner workflow after each submit and when the recap updates. */
  useEffect(() => {
    if (activeTab !== 'return' || isProcessingReturn || lostFound) return;
    let cancelled = false;
    const timers: number[] = [];
    const run = () => {
      if (cancelled || lostFoundOpenRef.current) return;
      returnBarcodeInputRef.current?.focus();
    };
    const frame = requestAnimationFrame(() => {
      run();
      timers.push(window.setTimeout(run, 0));
      timers.push(window.setTimeout(run, 100));
    });
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
      timers.forEach((id) => window.clearTimeout(id));
    };
  }, [activeTab, isProcessingReturn, returnResult, lostFound]);

  const showLostFound = useCallback(
    (
      fields: LostReturnFields & { itemId: string | null },
      summary: { title: string | null; barcode: string | null; patronName: string | null },
    ) => {
      setLostFoundPending(null);
      setLostFoundError(fields.itemId ? null : t('loans.lostFound.errorNotFound'));
      setLostFound({
        itemId: fields.itemId,
        wasLost: true,
        fineId: fields.fineId,
        fineStatus: fields.fineStatus,
        fineAmount: fields.fineAmount,
        paidAmount: fields.paidAmount,
      });
      setReturnResult({
        kind: 'lostFound',
        title: summary.title,
        barcode: summary.barcode,
        patronName: summary.patronName,
        foundAt: new Date().toISOString(),
        action: null,
        feeText: lostFoundPanelFee(null, fields, null, i18n.language, (key, values) =>
          String(t(key, values)),
        ),
      });
    },
    [i18n.language, t],
  );

  const openLostFound = useCallback(
    async (
      result: ItemCheckIn,
      hints: { itemId?: string | null; barcode?: string | null },
    ): Promise<boolean> => {
      if (!isLostReturn(result)) return false;
      let itemId = itemIdFromLostReturn(result, hints.itemId);
      const barcode = hints.barcode?.trim();
      if (!itemId && barcode) {
        try {
          const notice = await api.getItemByBarcode(barcode);
          itemId = itemIdFromCopies(notice.items, barcode);
        } catch {
          itemId = null;
        }
      }
      const loan = result.loan ?? null;
      showLostFound(
        {
          itemId,
          wasLost: true,
          fineId: result.fineId,
          fineStatus: result.fineStatus,
          fineAmount: result.fineAmount,
          paidAmount: result.paidAmount,
        },
        {
          title: loan?.biblio?.title?.trim() || null,
          barcode: (loan?.itemIdentification ?? barcode ?? '').trim() || null,
          patronName: readerName(loan?.user, canReadBorrowers),
        },
      );
      return true;
    },
    [canReadBorrowers, showLostFound],
  );

  const openLostFoundByBarcode = useCallback(
    async (barcode: string) => {
      const trimmed = barcode.trim();
      if (!trimmed || lostFoundLookupRef.current || lostFoundFlightRef.current) return;
      lostFoundLookupRef.current = true;
      setLostFoundLookup(true);
      try {
        let itemId: string | null = null;
        try {
          const notice = await api.getItemByBarcode(trimmed);
          itemId = itemIdFromCopies(notice.items, trimmed);
        } catch {
          itemId = null;
        }
        if (returnResult?.kind !== 'deskNotice' || returnResult.barcode !== trimmed) return;
        showLostFound({ itemId, wasLost: true }, { title: null, barcode: trimmed, patronName: null });
      } finally {
        lostFoundLookupRef.current = false;
        setLostFoundLookup(false);
      }
    },
    [returnResult, showLostFound],
  );

  const dismissLostFound = useCallback(() => {
    if (lostFoundFlightRef.current) return;
    setLostFound(null);
    setLostFoundError(null);
    setLostFoundPending(null);
    setReturnResult((current) => {
      if (current?.kind !== 'lostFound') return current;
      return {
        ...current,
        action: 'dismissed',
        feeText: lostFoundPanelFee('dismissed', lostFound ?? {}, null, i18n.language, (key, values) =>
          String(t(key, values)),
        ),
      };
    });
    showToast({ variant: 'info', message: t('loans.lostFound.dismissed') });
  }, [i18n.language, lostFound, showToast, t]);

  const resolveLostFoundChoice = useCallback(
    async (disposition: LostFoundDisposition) => {
      const itemId = lostFound?.itemId;
      if (!itemId || lostFoundFlightRef.current) return;
      lostFoundFlightRef.current = true;
      setLostFoundPending(disposition);
      setLostFoundError(null);
      try {
        const resolution = await api.resolveLostFound(itemId, { disposition });
        const feeText = lostFoundPanelFee(
          disposition,
          lostFound ?? {},
          resolution,
          i18n.language,
          (key, values) => String(t(key, values)),
        );
        const status = lostFoundResolutionMessage(resolution, i18n.language, (key, values) =>
          String(t(key, values)),
        );
        showToast({
          variant: 'success',
          message: disposition === 'leave' ? `${status} ${feeText}`.trim() : status,
        });
        setReturnResult((current) => {
          if (current?.kind !== 'lostFound') return current;
          return { ...current, action: disposition, feeText };
        });
        setLostFound(null);
        setLostFoundError(null);
      } catch (error: unknown) {
        const input = {
          status: getHttpStatus(error),
          code: getApiErrorCode(error),
          message: getApiErrorRawMessage(error),
        };
        if (classifyLostFoundError(input).kind === 'alreadyProcessed') {
          setLostFound(null);
          setLostFoundError(null);
          showToast({ variant: 'info', message: t('loans.lostFound.alreadyProcessed') });
          return;
        }
        setLostFoundError(lostFoundErrorMessage(input, (key) => t(key)));
      } finally {
        lostFoundFlightRef.current = false;
        setLostFoundPending(null);
      }
    },
    [i18n.language, lostFound, showToast, t],
  );

  const loadOverdue = useCallback(async () => {
    setOverdueLoading(true);
    setOverdueError(null);
    try {
      const data = await api.getOverdueLoans({ page: overduePage, perPage: overduePerPage });
      setOverdueData({ loans: data.loans, total: Number(data.total) });
    } catch (e: unknown) {
      setOverdueError(getApiErrorMessage(e, t) || t('loans.overdueLoadError'));
      setOverdueData(null);
    } finally {
      setOverdueLoading(false);
    }
  }, [t, overduePage, overduePerPage]);

  const handleOverdueReturn = useCallback(
    async (loanId: string) => {
      if (overdueLoanActionInFlightRef.current) return;
      overdueLoanActionInFlightRef.current = true;
      setOverdueLoanAction({ loanId, op: 'return' });
      try {
        const known = overdueData?.loans.find((loan) => loan.loanId === loanId);
        const result = await api.returnLoan(loanId);
        setOverdueError(null);
        setCheckoutScanAlert((current) =>
          checkoutAlertAfterSuccessfulReturn(current, {
            barcode: known?.itemBarcode,
            loanId,
          }),
        );
        setReturnScanAlert((current) =>
          current?.kind === 'onLoan' && known?.itemBarcode && returnBarcodeInput.trim() === known.itemBarcode
            ? null
            : current,
        );
        await loadOverdue();
        if (await openLostFound(result, { barcode: known?.itemBarcode })) return;
        const notice = returnToLenderNotice(result, t);
        setReturnToLenderMessage(notice);
        showToast({
          variant: 'success',
          message: withReturnToLenderNotice(t('loans.returnSuccess'), result, t),
        });
      } catch (e: unknown) {
        const msg = getApiErrorMessage(e, t) || t('loans.errorReturningLoan');
        setOverdueError(msg);
        showToast({ variant: 'error', message: msg });
      } finally {
        overdueLoanActionInFlightRef.current = false;
        setOverdueLoanAction(null);
      }
    },
    [loadOverdue, openLostFound, overdueData, returnBarcodeInput, showToast, t],
  );

  const rememberRenewalSetAside = (
    barcode: string | null,
    error: unknown,
    itemId?: string | null,
  ) => {
    const previous = renewalSetAsideRef.current;
    renewalSetAsideRef.current = {
      barcode: barcode ?? previous?.barcode ?? '',
      holdId: heldForOtherHoldId(error) ?? previous?.holdId ?? null,
      itemId: itemId ?? previous?.itemId ?? null,
    };
  };

  const handleOverdueRenew = async (loanId: string, force = false, overrideReason?: string) => {
    if (overdueLoanActionInFlightRef.current) return;
    overdueLoanActionInFlightRef.current = true;
    setOverdueLoanAction({ loanId, op: 'renew' });
    setOverdueError(null);
    if (!force) setRenewalPrompt(null);
    renewalRetryRef.current = null;
    try {
      const renewed = await api.renewLoan(loanId, newIdempotencyKey(), { force: force || undefined, overrideReason });
      setRenewalPrompt(null);
      await loadOverdue();
      showToast({
        variant: 'success',
        message: withDepositDueNotice(
          renewedUntilMessage(renewed.expiryAt, t, i18n.language),
          renewed,
          t,
          i18n.language,
        ),
      });
    } catch (e: unknown) {
      const barcode = overdueData?.loans.find((loan) => loan.loanId === loanId)?.itemBarcode ?? null;
      const held = overrideReasonRefusalMessage(e);
      if (force && held !== null) {
        setRenewalPrompt((current) =>
          current && current.kind !== 'sentence' ? { ...current, error: held } : current,
        );
        rememberRenewalSetAside(barcode, e);
        renewalRetryRef.current = (reason) => {
          void handleOverdueRenew(loanId, true, reason);
        };
        return;
      }
      const next = presentRenewalRefusal(e, { alreadyForced: force, t });
      if (next.kind === 'sentence') {
        setRenewalPrompt(null);
        setOverdueError(next.message);
        showToast({ variant: 'error', message: next.message });
      } else {
        rememberRenewalSetAside(barcode, e);
        renewalRetryRef.current = (reason) => {
          void handleOverdueRenew(loanId, true, reason);
        };
        setRenewalPrompt(next);
      }
    } finally {
      overdueLoanActionInFlightRef.current = false;
      setOverdueLoanAction(null);
    }
  };

  const applyPatronLoans = useCallback(
    async (options: { notify: boolean }) => {
      if (!selectedUser) return { ok: false as const, denied: false };
      const result = await readPatronLoans(() =>
        api.getUserLoans(selectedUser.id, currentUserLoansQuery(loansPage, BORROW_LOANS_PAGE_SIZE)),
      );
      if (result.ok) {
        setLoans(
          result.page.items.filter(isUserRecordLoan).map((loan) =>
            canReadBorrowers
              ? loan
              : { ...loan, user: deskBorrower(loan.user, false) },
          ) as LoanListItem[],
        );
        setLoansTotal(result.page.total);
        setLoansLoadError(null);
        setLoansAccessDenied(false);
        return result;
      }
      if (result.denied) {
        setLoans([]);
        setLoansTotal(0);
        setLoansLoadError(null);
        setLoansAccessDenied(true);
        return result;
      }
      setLoansAccessDenied(false);
      const msg = getApiErrorMessage(result.error, t) || t('loans.loansLoadError');
      setLoansLoadError(msg);
      if (options.notify) {
        showToast({ variant: 'error', message: msg });
      }
      return result;
    },
    [canReadBorrowers, loansPage, selectedUser, showToast, t],
  );

  const reloadSelectedUserLoans = useCallback(async () => {
    await applyPatronLoans({ notify: false });
  }, [applyPatronLoans]);

  const { data: claimsIdPage } = useQuery({
    queryKey: ['loans-claims-returned', 'ids'],
    queryFn: () => api.getClaimsReturned({ page: 1, perPage: 200 }),
  });
  const claimedLoanIds = useMemo(
    () => new Set((claimsIdPage?.items ?? []).map((row) => row.loanId)),
    [claimsIdPage],
  );

  const exceptionAction = useCirculationExceptionAction({
    setBusy: (busy) => {
      if (activeTab === 'overdue') {
        setOverdueLoanAction(busy);
      } else {
        setBorrowLoanAction(busy);
      }
    },
    onSuccess: async () => {
      await reloadSelectedUserLoans();
      if (activeTab === 'overdue') {
        await loadOverdue();
      }
    },
  });
  const exceptionOpen = exceptionAction.target != null;
  const borrowDeskBusy = borrowLoanAction != null || exceptionOpen;
  const overdueDeskBusy = overdueLoanAction != null || exceptionOpen;

  useEffect(() => {
    if (activeTab !== 'overdue') return;
    return deferFromEffect(() => { void loadOverdue(); });
  }, [activeTab, loadOverdue]);

  const overdueByUser = useMemo(() => {
    const loans = overdueData?.loans;
    if (!loans?.length) return [];
    const m = new Map<string, OverdueLoanInfo[]>();
    for (const loan of loans) {
      const key = patronId(loan.user?.id) ?? '';
      const list = m.get(key) ?? [];
      list.push(loan);
      m.set(key, list);
    }
    return Array.from(m.entries()).sort((a, b) => {
      const fa = readerIdentifierText(a[1][0]?.user, (number) => number);
      const fb = readerIdentifierText(b[1][0]?.user, (number) => number);
      return fa.localeCompare(fb, undefined, { sensitivity: 'base' });
    });
  }, [overdueData]);

  const overdueUserIds = overdueByUser.map(([uid]) => uid).join('\0');
  const [prevOverdueUserIds, setPrevOverdueUserIds] = useState(overdueUserIds);
  if (overdueUserIds !== prevOverdueUserIds) {
    setPrevOverdueUserIds(overdueUserIds);
    setOverdueOpenByUser((prev) => {
      const next: Record<string, boolean> = {};
      for (const [uid, loans] of overdueByUser) {
        if (loans.length > 0 && prev[uid]) {
          next[uid] = true;
        }
      }
      return next;
    });
  }

  const overdueTotalPages = overdueData
    ? Math.max(1, Math.ceil(overdueData.total / overduePerPage))
    : 1;

  const executeSendReminders = async (dryRun: boolean) => {
    setReminderSending(true);
    setReminderReport(null);
    setOverdueError(null);
    try {
      const report = await api.sendOverdueReminders({ dryRun: dryRun });
      setReminderReport(report);
      if (!dryRun) void loadOverdue();
    } catch (e: unknown) {
      setOverdueError(getApiErrorMessage(e, t) || t('loans.sendRemindersError'));
    } finally {
      setReminderSending(false);
    }
  };

  const handleSendReminders = async (dryRun: boolean) => {
    if (!dryRun) {
      setSendRemindersConfirmOpen(true);
      return;
    }
    await executeSendReminders(true);
  };

  const userSearchSeqRef = useRef(0);

  const userQuery = userSearchDraft.trim();
  const visibleUserResults = userQuery ? userSearchResults : [];
  const visibleUserSearching = userQuery ? isSearchingUsers : false;

  useEffect(() => {
    const query = userSearchDraft.trim();
    if (!deskNameSearchVisible(canReadBorrowers) || !query) {
      userSearchSeqRef.current += 1;
      return;
    }

    const timer = window.setTimeout(() => {
      const seq = ++userSearchSeqRef.current;
      setIsSearchingUsers(true);
      setUserSearchError(null);
      void (async () => {
        try {
          const response = await api.getUsers({
            name: query,
            perPage: 10,
          });
          if (seq !== userSearchSeqRef.current) return;
          setUserSearchResults(response.items);
        } catch (error) {
          if (seq !== userSearchSeqRef.current) return;
          setUserSearchResults([]);
          const msg = getApiErrorMessage(error, t) || t('loans.userSearchError');
          setUserSearchError(msg);
          showToast({ variant: 'error', message: msg });
        } finally {
          if (seq === userSearchSeqRef.current) {
            setIsSearchingUsers(false);
          }
        }
      })();
    }, 300);

    return () => window.clearTimeout(timer);
  }, [canReadBorrowers, userSearchDraft, userSearchRetry, showToast, t]);

  useEffect(() => {
    if (!selectedUser) {
      return;
    }

    const loadUserLoans = async () => {
      setIsLoadingLoans(true);
      try {
        await applyPatronLoans({ notify: true });
      } finally {
        setIsLoadingLoans(false);
      }
    };

    return deferFromEffect(() => { void loadUserLoans(); });
  }, [selectedUser, loansPage, loansReloadToken, applyPatronLoans]);

  const [prevSelectedUser, setPrevSelectedUser] = useState(selectedUser);
  if (selectedUser !== prevSelectedUser) {
    setPrevSelectedUser(selectedUser);
    setLoansAccessDenied(false);
    if (selectedUser) setIsLoadingLoans(true);
    if (!selectedUser) {
      setLoans([]);
      setLoansTotal(0);
      setLoansLoadError(null);
    }
  }

  const handleUserSelect = async (user: UserShort) => {
    if (patronLookupInFlightRef.current) return;
    patronLookupInFlightRef.current = true;
    setIsPatronLookupBusy(true);
    setUserSelectError(null);
    try {
      const fullUser = await api.getUser(user.id);
      selectDeskReader(fullUser);
      setPatronIdentity(
        deskPatronIdentity({
          id: fullUser.id,
          barcode: fullUser.barcode,
          firstname: fullUser.firstname,
          lastname: fullUser.lastname,
          accountType: fullUser.accountType,
          email: fullUser.email,
          phone: fullUser.phone,
          login: fullUser.login,
        }),
      );
      setDeskLoanStatus(null);
      setCardScanError(null);
      setLoansPage(1);
      setLastCheckout(null);
      setUserSearchDraft('');
      setUserSearchResults([]);
      const card = fullUser.barcode?.trim();
      if (card) {
        try {
          const byCard = await api.getUserByCard(card);
          setDeskLoanStatus(byCard.loanStatus);
        } catch {
          setDeskLoanStatus(null);
        }
      }
    } catch (error) {
      const msg = getApiErrorMessage(error, t) || t('loans.userLoadError');
      setUserSelectError(msg);
      showToast({ variant: 'error', message: msg });
    } finally {
      patronLookupInFlightRef.current = false;
      setIsPatronLookupBusy(false);
    }
  };

  const handleUserBarcodeScan = async (barcode: string) => {
    if (!barcode.trim() || patronLookupInFlightRef.current) return;
    patronLookupInFlightRef.current = true;
    setIsPatronLookupBusy(true);
    try {
      const found = await api.getUserByCard(barcode.trim());
      const scanned = patronFromCardScan(found);
      selectDeskReader(scanned.user);
      setPatronIdentity(scanned.identity);
      setDeskLoanStatus(scanned.loanStatus);
      setCardScanError(null);
      if (!canReadBorrowers) {
        setLoans([]);
        setLoansTotal(0);
        setLoansLoadError(null);
        setLoansAccessDenied(false);
      }
      setLoansPage(1);
      setLastCheckout(null);
      setUserSearchDraft('');
      setUserSearchResults([]);
      if (userBarcodeInputRef.current) {
        userBarcodeInputRef.current.value = '';
      }
    } catch (error) {
      const presented = presentCardScanError(
        getHttpStatus(error),
        getApiErrorMessage(error, t) || t('loans.errorFindingUser'),
        t('loans.unknownCard'),
      );
      setCardScanError(presented.message);
    } finally {
      patronLookupInFlightRef.current = false;
      setIsPatronLookupBusy(false);
    }
  };

  const handleBorrow = async (specimenBarcode: string, force = false, overrideReason?: string) => {
    if (!selectedUser || !specimenBarcode.trim()) {
      throw new Error(t('loans.noUserSelected'));
    }

    const barcode = specimenBarcode.trim();
    const snapshot = await readDeskCopy(barcode);
    const alert = deskScanAlert(snapshot, {
      patronId: selectedUser.id,
      revealIdentity: canReadBorrowers,
    });
    if (alert && checkoutScanStopsLoan(alert)) {
      const shown =
        alert.kind === 'onLoan'
          ? {
              kind: 'onLoan' as const,
              patronName: null,
              otherBorrower: otherBorrowerFromPerson({
                userId: snapshot?.loanUserId,
                cardNumber: snapshot?.loanUserCard,
                initials: snapshot?.loanUserInitials,
                firstname: snapshot?.loanUserFirstname,
                lastname: snapshot?.loanUserLastname,
                fullName: snapshot?.loanUserName,
              }),
            }
          : alert;
      setCheckoutScanAlert({ ...shown, barcode, loanId: snapshot?.loanId ?? null });
      throw new CheckoutScanNoticeShown();
    }
    setCheckoutScanAlert(null);

    try {
      const forceFlag = checkoutForceFlag(snapshot?.availability, force);
      const reason = forceFlag ? trimmedOverrideReason(overrideReason) : undefined;
      const created = await api.createLoan({
        userId: selectedUser.id,
        itemIdentification: barcode,
        force: forceFlag,
        ...(reason ? { overrideReason: reason } : {}),
      }, newIdempotencyKey());
      const loaded = await applyPatronLoans({ notify: false });
      const matched = loaded.ok
        ? loaded.page.items.find((loan) => loan.id === created.id)
        : undefined;
      const recap = checkoutRecap({
        created,
        barcode,
        patronName: deskPatronLabel(patronIdentity, (number) => t('loans.readerCardNo', { number })),
        noTitle: t('loans.noTitle'),
        listed: matched ?? null,
      });
      setLastCheckout({ ...recap, dueDateCappedByDeposit: created.dueDateCappedByDeposit === true });
      showToast({
        variant: 'success',
        message: withDepositDueNotice(
          t('loans.checkoutSuccess', { title: recap.title, patron: recap.patronName }),
          created,
          t,
          i18n.language,
        ),
      });
    } catch (error: unknown) {
      const rawMessage = getApiErrorRawMessage(error) ?? '';
      const conflict = checkoutNoticeForConflict(error, {
        patronId: selectedUser.id,
        snapshot,
        listedLoanId: loans.find((loan) => (loan.itemIdentification ?? '').trim() === barcode)?.id ?? null,
      });
      if (conflict) {
        setCheckoutScanAlert({ ...conflict, barcode });
        throw new CheckoutScanNoticeShown();
      }
      if (isWeedingServerMessage(rawMessage)) {
        throw error;
      }
      const view = presentCheckoutRefusal(error, {
        canForce: canForceLoan,
        alreadyForced: force,
        t,
      });
      if (view.kind === 'setAside') {
        setToPickAside({
          barcode,
          holdId: heldForOtherHoldId(error),
          itemId: snapshot?.itemId?.trim() || null,
          message: view.message,
          force: view.force,
          reasonRequired: view.reasonRequired,
          error: null,
        });
        throw new CheckoutDeclined();
      }
      if (view.kind === 'scan') {
        setCheckoutScanAlert({ ...view.alert, barcode });
        throw new CheckoutScanNoticeShown();
      }
      if (view.kind === 'status') {
        throw error;
      }
      if (view.kind === 'force') {
        setBarcodeInput('');
        setForceBorrowDialog({
          message: view.message,
          barcode,
          reasonRequired: view.reasonRequired,
          error: null,
        });
        throw new CheckoutDeclined();
      }
      throw error;
    }
  };

  const handleReturn = async (loanId: string) => {
    if (borrowLoanActionInFlightRef.current) return;
    borrowLoanActionInFlightRef.current = true;
    setBorrowLoanAction({ loanId, op: 'return' });
    setRenewError('');
    try {
      const known = loans.find((loan) => loan.id === loanId);
      const result = await api.returnLoan(loanId);
      setCheckoutScanAlert((current) =>
        checkoutAlertAfterSuccessfulReturn(current, {
          barcode: known?.itemIdentification,
          loanId,
        }),
      );
      if (selectedUser) {
        await applyPatronLoans({ notify: false });
      }
      if (
        await openLostFound(result, {
          itemId: known?.itemId,
          barcode: known?.itemIdentification,
        })
      ) {
        return;
      }
      const fallback = lenderNameFromCopies(known?.biblio?.items, known?.itemId);
      const notice = returnToLenderNotice(result, t, fallback);
      setReturnToLenderMessage(notice);
      showToast({
        variant: 'success',
        message: withReturnToLenderNotice(t('loans.returnSuccess'), result, t, fallback),
      });
    } catch (error: unknown) {
      const msg = getApiErrorMessage(error, t) || t('loans.errorReturningLoan');
      setRenewError(msg);
      showToast({ variant: 'error', message: msg });
    } finally {
      borrowLoanActionInFlightRef.current = false;
      setBorrowLoanAction(null);
    }
  };

  const handleReturnByBarcode = async (specimenBarcode: string) => {
    if (!specimenBarcode.trim()) {
      throw new Error(t('loans.barcodeRequired'));
    }

    setIsProcessingReturn(true);
    setReturnError('');
    setReturnResult(null);
    setReturnScanAlert(null);
    setReturnToLenderMessage(null);

    const barcode = specimenBarcode.trim();
    const snapshot = await readDeskCopy(barcode);
    const preview = deskScanAlert(snapshot, { revealIdentity: canReadBorrowers });

    try {
      const result = await api.returnLoanByBarcode(barcode);
      const fallback = snapshot?.lenderName ?? null;
      const lenderNotice = returnToLenderNotice(result, t, fallback);
      setReturnToLenderMessage(lenderNotice);
      const returnedMessage = withReturnToLenderNotice(t('loans.returnSuccess'), result, t, fallback);
      const pick = interpretPickCheckIn(result);
      const feedback = returnScanFeedback(result, { pick: Boolean(pick), hasLoan: Boolean(result.loan) });
      if (feedback.toast === 'none' && feedback.notice) {
        setReturnScanAlert(null);
        setReturnResult({ kind: 'deskNotice', notice: feedback.notice, putBack: false, barcode });
        setReturnBarcodeInput('');
        return;
      }
      setReturnScanAlert(deskScanAlertAfterReturn(result, canReadBorrowers));
      setCheckoutScanAlert((current) =>
        checkoutAlertAfterSuccessfulReturn(current, {
          barcode,
          loanId: result.loan?.id ?? null,
        }),
      );
      if (await openLostFound(result, { barcode })) {
        setReturnBarcodeInput('');
        return;
      }
      if (pick) {
        const notice = await resolvePickNotice(pick);
        setReturnResult({ kind: 'pick', notice, deskNotice: feedback.notice });
        const message =
          notice.kind === 'ready'
            ? t('loans.pickReady', { patron: notice.patronName?.trim() || t('loans.pickPatronUnknown') })
            : t('loans.pickTransit', { site: notice.siteName?.trim() || t('loans.pickSiteUnknown') });
        showToast({
          variant: 'success',
          message: lenderNotice ? `${message} ${lenderNotice}` : message,
        });
      } else if (result.loan) {
        setReturnResult({ kind: 'loan', loan: result.loan });
        showToast({ variant: 'success', message: returnedMessage });
      } else if (feedback.toast === 'info' && feedback.notice) {
        setReturnResult({ kind: 'deskNotice', notice: feedback.notice, putBack: !lenderNotice, barcode });
        const infoMessage = returnNoticeMessage(feedback.notice, (key, values) => String(t(key, values)), {
          putBack: !lenderNotice,
        });
        showToast({
          variant: 'info',
          message: lenderNotice ? `${infoMessage} ${lenderNotice}` : infoMessage,
        });
      } else {
        setReturnResult(null);
        showToast({ variant: 'success', message: returnedMessage });
      }
      setReturnBarcodeInput('');
    } catch (error: unknown) {
      setReturnScanAlert(preview);
      const msg = getApiErrorMessage(error, t) || t('loans.errorReturningLoan');
      setReturnError(msg);
      showToast({ variant: 'error', message: msg });
      requestAnimationFrame(() => {
        const el = returnBarcodeInputRef.current;
        el?.focus();
        el?.select();
      });
    } finally {
      setIsProcessingReturn(false);
    }
  };

  const loanColumns = [
    {
      key: 'title',
      header: t('loans.document'),
      render: (loan: LoanListItem) => {
        const specs = loan.biblio?.items;
        const spec = specs?.length
          ? (specs.find((s) => s.id === loan.itemId) ?? specs.find((s) => s.borrowed) ?? specs[0])
          : null;
        const specimenBarcode = spec ? (spec.barcode ?? spec.id) : loan.itemIdentification;
        return (
          <div className="flex items-start gap-3 min-w-0">
            <LoanMediaTypeBadge mediaType={loan.biblio.mediaType} size="table" />
            <div className="min-w-0">
              <p className="font-medium text-gray-900 dark:text-white">
                {loan.biblio.title || t('loans.noTitle')}
              </p>
              <div className="text-sm text-gray-500 dark:text-gray-400 space-y-0.5 mt-0.5">
                {loan.biblio.isbn && (
                  <p>
                    {t('items.isbn')}: <span className="font-mono">{formatIsbnDisplay(loan.biblio.isbn)}</span>
                  </p>
                )}
                <p>{t('items.barcode')}: <span className="font-mono">{specimenBarcode ?? '-'}</span></p>
                <DepositBadge deposit={spec?.deposit} />
              </div>
            </div>
          </div>
        );
      },
    },
    {
      key: 'date',
      header: t('loans.borrowDate'),
      render: (loan: LoanListItem) =>
        new Date(loan.startDate).toLocaleDateString(i18n.language),
    },
    {
      key: 'expiryAt',
      header: t('loans.dueDate'),
      render: (loan: LoanListItem) => (
        <div className="flex items-center gap-2">
          <span>{new Date(loan.expiryAt).toLocaleDateString(i18n.language)}</span>
          {loan.isOverdue && <Badge variant="danger">{t('loans.overdue')}</Badge>}
        </div>
      ),
    },
    {
      key: 'renews',
      header: t('loans.renewals'),
      render: (loan: LoanListItem) => loan.nbRenews,
    },
    {
      key: 'actions',
      header: t('common.actions'),
      align: 'right' as const,
      render: (loan: LoanListItem) => {
        const claimed = isClaimedReturnedLoan(loan, claimedLoanIds);
        const busy = borrowLoanAction?.loanId === loan.id ? borrowLoanAction.op : null;
        return (
        <div className="flex flex-col items-end gap-2">
          <div className="flex items-center justify-end gap-2 flex-wrap">
            {!claimed && (
              <Button
                size="sm"
                variant="ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  void handleRenewLoan(loan.id);
                }}
                leftIcon={<RotateCcw className="h-4 w-4" />}
                isLoading={busy === 'renew'}
                disabled={borrowDeskBusy}
              >
                {t('loans.renew')}
              </Button>
            )}
            <Button
              size="sm"
              variant="primary"
              onClick={(e) => {
                e.stopPropagation();
                void handleReturn(loan.id);
              }}
              leftIcon={<Check className="h-4 w-4" />}
              isLoading={busy === 'return'}
              disabled={borrowDeskBusy}
            >
              {t('loans.return')}
            </Button>
          </div>
          <LoanExceptionActions
            loanId={loan.id}
            loanLabel={loan.biblio.title || undefined}
            claimedReturned={claimed}
            disabled={borrowDeskBusy}
            busyKind={busy && busy !== 'return' && busy !== 'renew' ? busy : null}
            onOpen={(kind, label) => exceptionAction.open({ loanId: loan.id, kind, label })}
          />
        </div>
        );
      },
    },
  ];

  const completeRenew = async (
    busyId: string,
    request: (force: boolean, overrideReason?: string) => Promise<{ expiryAt: string; dueDateCappedByDeposit?: boolean }>,
    force = false,
    barcode: string | null = null,
    overrideReason?: string,
    itemId: string | null = null,
  ): Promise<boolean> => {
    if (borrowLoanActionInFlightRef.current) return false;
    borrowLoanActionInFlightRef.current = true;
    setBorrowLoanAction({ loanId: busyId, op: 'renew' });
    setRenewError('');
    if (!force) setRenewalPrompt(null);
    renewalRetryRef.current = null;
    try {
      const renewed = await request(force, overrideReason);
      setRenewalPrompt(null);
      if (selectedUser) {
        await applyPatronLoans({ notify: false });
      }
      showToast({
        variant: 'success',
        message: withDepositDueNotice(
          renewedUntilMessage(renewed.expiryAt, t, i18n.language),
          renewed,
          t,
          i18n.language,
        ),
      });
      return true;
    } catch (error: unknown) {
      const held = overrideReasonRefusalMessage(error);
      if (force && held !== null) {
        setRenewalPrompt((current) =>
          current && current.kind !== 'sentence' ? { ...current, error: held } : current,
        );
        rememberRenewalSetAside(barcode, error, itemId);
        renewalRetryRef.current = (reason) => {
          void completeRenew(busyId, request, true, barcode, reason, itemId);
        };
        return false;
      }
      const next = presentRenewalRefusal(error, { alreadyForced: force, t });
      if (next.kind === 'sentence') {
        setRenewalPrompt(null);
        setRenewError(next.message);
        showToast({ variant: 'error', message: next.message });
      } else {
        rememberRenewalSetAside(barcode, error, itemId);
        renewalRetryRef.current = (reason) => {
          void completeRenew(busyId, request, true, barcode, reason, itemId);
        };
        setRenewalPrompt(next);
      }
      return false;
    } finally {
      borrowLoanActionInFlightRef.current = false;
      setBorrowLoanAction(null);
    }
  };

  const handleRenewLoan = (loanId: string) => {
    const loan = loans.find((row) => row.id === loanId);
    return completeRenew(
      loanId,
      (force, overrideReason) => api.renewLoan(loanId, newIdempotencyKey(), { force: force || undefined, overrideReason }),
      false,
      loan?.itemIdentification ?? null,
      undefined,
      loan?.itemId ?? null,
    );
  };

  const renewFromCheckout = async (alert: CheckoutScanAlert) => {
    if (alert.kind !== 'alreadyOnLoan') return;
    const ok = alert.loanId
      ? await completeRenew(
          alert.loanId,
          (force, overrideReason) =>
            api.renewLoan(alert.loanId as string, newIdempotencyKey(), { force: force || undefined, overrideReason }),
          false,
          alert.barcode,
        )
      : await completeRenew(
          alert.barcode,
          (force, overrideReason) =>
            api.renewLoanByBarcode(alert.barcode, newIdempotencyKey(), { force: force || undefined, overrideReason }),
          false,
          alert.barcode,
        );
    if (ok) setCheckoutScanAlert(null);
  };

  const loansSortedByStart = useMemo(() => sortLoansByStartDateAsc(loans), [loans]);

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h1 className="text-2xl font-bold text-gray-900 dark:text-white">
          {t('loans.title')}
        </h1>
        <p className="text-gray-500 dark:text-gray-400 mt-1">
          {t('loans.subtitle')}
        </p>
      </div>

      {/* Tabs */}
      <div className="border-b border-gray-200 dark:border-gray-800">
        <nav className="-mb-px flex space-x-8 overflow-x-auto">
          <button
            onClick={() => {
              setActiveTab('borrow');
              setReturnResult(null);
              setReturnError('');
              setReminderReport(null);
            }}
            className={`py-4 px-1 border-b-2 font-medium text-sm ${
              activeTab === 'borrow'
                ? 'border-amber-500 text-amber-700 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <BookMarked className="h-5 w-5" />
              {t('loans.tabBorrow')}
            </div>
          </button>
          <button
            onClick={() => {
              setActiveTab('return');
              setReturnResult(null);
              setReturnError('');
              setReminderReport(null);
            }}
            className={`py-4 px-1 border-b-2 font-medium text-sm ${
              activeTab === 'return'
                ? 'border-amber-500 text-amber-700 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <Check className="h-5 w-5" />
              {t('loans.tabReturn')}
            </div>
          </button>
          <button
            onClick={() => {
              setActiveTab('overdue');
              setReturnResult(null);
              setReturnError('');
              setReminderReport(null);
            }}
            className={`py-4 px-1 border-b-2 font-medium text-sm ${
              activeTab === 'overdue'
                ? 'border-amber-500 text-amber-700 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <Bell className="h-5 w-5" />
              {t('loans.overdueTab')}
            </div>
          </button>
          <button
            onClick={() => {
              setActiveTab('claims');
              setReturnResult(null);
              setReturnError('');
              setReminderReport(null);
            }}
            className={`py-4 px-1 border-b-2 font-medium text-sm ${
              activeTab === 'claims'
                ? 'border-amber-500 text-amber-700 dark:text-amber-400'
                : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300 dark:text-gray-400 dark:hover:text-gray-300'
            }`}
          >
            <div className="flex items-center gap-2">
              <ClipboardList className="h-5 w-5" />
              {t('loans.exceptions.tabClaims')}
              {(claimsIdPage?.total ?? 0) > 0 ? (
                <Badge variant="warning" size="sm">{claimsIdPage?.total}</Badge>
              ) : null}
            </div>
          </button>
        </nav>
      </div>

      {/* Borrow Tab */}
      {activeTab === 'borrow' && (
        <div className="space-y-6">
          {!selectedUser && (
          <Card>
            <div className="flex flex-col gap-6">
              <div className="flex gap-4">
                <div
                  className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-amber-500 text-base font-bold text-white shadow-md dark:bg-amber-600"
                  aria-hidden
                >
                  1
                </div>
                <div className="min-w-0 flex-1 pt-0.5">
                  <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                    {t('loans.selectUser')}
                  </h3>
                  <p className="mt-1 text-sm text-gray-500 dark:text-gray-400 leading-relaxed">
                    {t(canReadBorrowers ? 'loans.selectUserHint' : 'loans.selectUserHintScan')}
                  </p>
                </div>
              </div>

              <div className="sm:ml-2 sm:border-l-2 sm:border-amber-200 sm:pl-6 sm:dark:border-amber-800">
                  <div className="space-y-5">
                    {/* User barcode scan */}
                    <div>
                      <label className={formLabelClass()}>
                        {t('loans.scanUserCard')}
                      </label>
                      <form
                        onSubmit={(e) => {
                          e.preventDefault();
                          if (isPatronLookupBusy) return;
                          const barcode = (e.currentTarget.elements.namedItem('userBarcode') as HTMLInputElement)?.value;
                          if (barcode) {
                            void handleUserBarcodeScan(barcode);
                          }
                        }}
                      >
                        <BarcodeScanField
                          ref={userBarcodeInputRef}
                          name="userBarcode"
                          placeholder={t('loans.scanOrEnterCard')}
                          leftIcon={<CreditCard className="h-4 w-4" />}
                          autoFocus
                          disabled={isPatronLookupBusy}
                          aria-busy={isPatronLookupBusy}
                          scannerTitle={t('loans.scanUserCard')}
                          onCameraScan={(barcode) => {
                            if (isPatronLookupBusy) return;
                            if (userBarcodeInputRef.current) {
                              userBarcodeInputRef.current.value = barcode;
                            }
                            void handleUserBarcodeScan(barcode);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === 'Enter') {
                              e.preventDefault();
                              if (isPatronLookupBusy) return;
                              const barcode = (e.target as HTMLInputElement).value;
                              if (barcode) {
                                void handleUserBarcodeScan(barcode);
                              }
                            }
                          }}
                        />
                      </form>
                      {cardScanError ? (
                        <div
                          role="alert"
                          className="mt-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800"
                        >
                          <p className="text-sm font-medium text-red-800 dark:text-red-200">{cardScanError}</p>
                        </div>
                      ) : null}
                    </div>

                    <DeskNameSearch
                      canReadUsers={canReadBorrowers}
                      value={userSearchDraft}
                      onChange={(next) => {
                        setUserSearchDraft(next);
                        setUserSearchError(null);
                        setUserSelectError(null);
                      }}
                      items={visibleUserResults}
                      onSelect={(user) => void handleUserSelect(user)}
                      loading={visibleUserSearching}
                      disabled={isPatronLookupBusy}
                      error={userSearchError}
                      selectError={userSelectError}
                      onRetry={() => setUserSearchRetry((n) => n + 1)}
                    />
                  </div>
              </div>
            </div>
          </Card>
          )}

          {selectedUser && (
            <>
              <Card
                padding="sm"
                className="sticky top-0 z-20 border-amber-200/80 bg-white/95 shadow-md backdrop-blur-sm dark:border-amber-900/40 dark:bg-gray-900/95"
              >
                <div className="flex flex-col gap-3">
                  <div className="flex items-start justify-between gap-3">
                    {patronIdentity ? (
                      <DeskPatronStatus
                        identity={patronIdentity}
                        linked={canReadBorrowers}
                        loanStatus={deskLoanStatus}
                      />
                    ) : null}
                    <Button
                      variant="ghost"
                      size="sm"
                      className="shrink-0"
                      onClick={() => {
                        selectDeskReader(null);
                        setPatronIdentity(null);
                        setDeskLoanStatus(null);
                        setCardScanError(null);
                        setRenewalPrompt(null);
                        renewalRetryRef.current = null;
                        renewalSetAsideRef.current = null;
                        setLoans([]);
                        setLoansTotal(0);
                        setLoansPage(1);
                        setLoansAccessDenied(false);
                        setUserSearchDraft('');
                        setBarcodeInput('');
                        setLastCheckout(null);
                        setCheckoutScanAlert(null);
                        setRenewError('');
                        setLoansLoadError(null);
                      }}
                      leftIcon={<X className="h-4 w-4" />}
                    >
                      {t('common.clear')}
                    </Button>
                  </div>
                  <div className="max-w-2xl">
                    <p className="mb-1.5 text-xs font-medium text-gray-600 dark:text-gray-400">
                      {t('loans.borrow')}
                    </p>
                    <BorrowForm
                      formId="loans-borrow-inline-form"
                      variant="inline"
                      onBorrow={handleBorrow}
                      barcodeInput={barcodeInput}
                      setBarcodeInput={setBarcodeInput}
                      barcodeInputRef={barcodeInputRef}
                      onLoadingChange={setIsBorrowLoading}
                      isLoading={isBorrowLoading}
                    />
                    {checkoutScanAlert ? (
                      <DeskScanNotice
                        alert={checkoutScanAlert}
                        canReadBorrowers={canReadBorrowers}
                        renewing={borrowLoanAction?.op === 'renew'}
                        onReturn={
                          checkoutScanAlert.kind === 'onLoan'
                            ? () => {
                                setReturnBarcodeInput(checkoutScanAlert.barcode);
                                setReturnError('');
                                setReturnResult(null);
                                setReturnScanAlert(null);
                                setActiveTab('return');
                              }
                            : undefined
                        }
                        onRenew={
                          checkoutScanAlert.kind === 'alreadyOnLoan'
                            ? () => {
                                void renewFromCheckout(checkoutScanAlert);
                              }
                            : undefined
                        }
                      />
                    ) : null}
                  </div>
                </div>
              </Card>
              {lastCheckout && (
                <div>
                  <p className="mb-2 text-sm font-medium text-gray-700 dark:text-gray-300">
                    {t('loans.checkoutSummaryTitle')}
                  </p>
                  <CheckoutRecapPanel recap={lastCheckout} locale={i18n.language} t={t} />
                </div>
              )}

              {/* Step 3 — Active loans list */}
              <Card padding="none" className="flex flex-col min-h-0">
                <div className="p-4 sm:p-6 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
                  <div className="flex gap-4">
                    <div
                      className="hidden sm:flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-gray-200 text-base font-bold text-gray-700 dark:bg-gray-700 dark:text-gray-200"
                      aria-hidden
                    >
                      3
                    </div>
                    <div className="min-w-0 flex-1 pt-0.5">
                      <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                        {t('loans.activeLoans')}
                      </h3>
                      <p className="mt-1 text-sm text-gray-500 dark:text-gray-400 sm:hidden">
                        {deskPatronLabel(patronIdentity, (number) => t('loans.readerCardNo', { number }))}
                      </p>
                    </div>
                  </div>
                  {renewError && (
                    <RenewalRefusalNotice
                      part="alert"
                      view={{ kind: 'sentence', message: renewError }}
                      onForce={() => undefined}
                      onDismiss={() => undefined}
                      onSetAside={() => undefined}
                    />
                  )}
                  {loansLoadError && (
                    <div
                      role="alert"
                      className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 flex flex-col sm:flex-row sm:items-center gap-2"
                    >
                      <p className="text-sm font-medium text-red-800 dark:text-red-200 flex-1">{loansLoadError}</p>
                      <Button
                        type="button"
                        size="sm"
                        variant="secondary"
                        onClick={() => setLoansReloadToken((n) => n + 1)}
                      >
                        {t('common.retry')}
                      </Button>
                    </div>
                  )}
                </div>
                <ScrollableListRegion aria-label={t('loans.activeLoans')}>
                  {isLoadingLoans && !loans.length && !loansAccessDenied ? (
                    <ListSkeleton rows={6} />
                  ) : (
                    <PatronLoansOutcome
                      denied={loansAccessDenied}
                      itemCount={loansSortedByStart.length}
                      emptyMessage={t('loans.noLoans')}
                    >
                    <ResponsiveRecordList
                      desktop={
                        <Table
                          columns={loanColumns}
                          data={loansSortedByStart}
                          keyExtractor={(loan) => loan.id}
                          isLoading={false}
                          emptyMessage={t('loans.noLoans')}
                        />
                      }
                      mobile={
                        loans.length === 0 ? (
                          <div className="flex flex-col items-center justify-center py-12 text-gray-500 dark:text-gray-400 px-4">
                            {t('loans.noLoans')}
                          </div>
                        ) : (
                          <div className="rounded-lg border border-gray-200 dark:border-gray-800 overflow-hidden bg-white dark:bg-gray-900 mx-2 sm:mx-4 mb-2">
                            {loansSortedByStart.map((loan) => (
                              <ActiveLoanCard
                                key={loan.id}
                                loan={loan}
                                onRenew={() => void handleRenewLoan(loan.id)}
                                onReturn={() => void handleReturn(loan.id)}
                                renewLoading={
                                  borrowLoanAction?.loanId === loan.id && borrowLoanAction.op === 'renew'
                                }
                                returnLoading={
                                  borrowLoanAction?.loanId === loan.id && borrowLoanAction.op === 'return'
                                }
                                actionsDisabled={borrowDeskBusy}
                                hideRenew={isClaimedReturnedLoan(loan, claimedLoanIds)}
                                exceptionActions={
                                  <LoanExceptionActions
                                    loanId={loan.id}
                                    loanLabel={loan.biblio.title || undefined}
                                    claimedReturned={isClaimedReturnedLoan(loan, claimedLoanIds)}
                                    disabled={borrowDeskBusy}
                                    busyKind={
                                      borrowLoanAction?.loanId === loan.id &&
                                      borrowLoanAction.op !== 'return' &&
                                      borrowLoanAction.op !== 'renew'
                                        ? borrowLoanAction.op
                                        : null
                                    }
                                    onOpen={(kind, label) => exceptionAction.open({ loanId: loan.id, kind, label })}
                                  />
                                }
                              />
                            ))}
                          </div>
                        )
                      }
                    />
                    </PatronLoansOutcome>
                  )}
                </ScrollableListRegion>
                {loansTotal > 0 && (
                  <div className="p-4 border-t border-gray-200 dark:border-gray-800 flex-shrink-0">
                    <Pagination
                      currentPage={loansPage}
                      totalPages={Math.max(1, Math.ceil(loansTotal / BORROW_LOANS_PAGE_SIZE))}
                      onPageChange={setLoansPage}
                    />
                  </div>
                )}
              </Card>
            </>
          )}
        </div>
      )}

      {/* Return Tab */}
      {activeTab === 'return' && (
        <div className="space-y-6">
          {/* Step 1 — Scan return */}
          <Card className="border-emerald-200/80 shadow-sm dark:border-emerald-900/40">
            <div className="flex flex-col gap-5 sm:flex-row sm:gap-6">
              <div
                className="flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-emerald-600 text-base font-bold text-white shadow-md dark:bg-emerald-500"
                aria-hidden
              >
                1
              </div>
              <div className="min-w-0 flex-1 space-y-4">
                <div>
                  <h3 className="text-lg font-semibold text-gray-900 dark:text-white flex items-center gap-2 flex-wrap">
                    <Check className="h-5 w-5 text-emerald-600 dark:text-emerald-400 shrink-0" />
                    {t('loans.returnLoan')}
                  </h3>
                  <p className="mt-1 text-sm text-gray-500 dark:text-gray-400 leading-relaxed max-w-2xl">
                    {t('loans.returnSectionHint')}
                  </p>
                </div>
                <div className="sm:ml-2 sm:border-l-2 sm:border-emerald-200 sm:pl-6 sm:dark:border-emerald-800">
                  <form
                    onSubmit={(e) => {
                      e.preventDefault();
                      if (returnBarcodeInput.trim() && !isProcessingReturn && lostFound === null) {
                        void handleReturnByBarcode(returnBarcodeInput);
                      }
                    }}
                    className="space-y-3 max-w-2xl"
                  >
                    <BarcodeScanField
                      ref={returnBarcodeInputRef}
                      value={returnBarcodeInput}
                      onChange={(e) => {
                        setReturnBarcodeInput(e.target.value);
                        setReturnError('');
                        setReturnResult(null);
                      }}
                      placeholder={t('loans.scanOrEnterBarcode')}
                      leftIcon={<BookOpen className="h-4 w-4" />}
                      autoFocus
                      disabled={isProcessingReturn || lostFound !== null}
                      aria-busy={isProcessingReturn}
                      scannerTitle={t('loans.returnLoan')}
                      onCameraScan={(barcode) => {
                        setReturnBarcodeInput(barcode);
                        setReturnError('');
                        setReturnResult(null);
                        if (!isProcessingReturn && lostFound === null) {
                          void handleReturnByBarcode(barcode);
                        }
                      }}
                      rightIcon={
                        <Button
                          type="submit"
                          size="icon"
                          variant="primary"
                          disabled={isProcessingReturn || lostFound !== null || !returnBarcodeInput.trim()}
                          aria-label={t('common.validate')}
                          className="bg-emerald-600 hover:bg-emerald-700 dark:bg-emerald-500 dark:hover:bg-emerald-600"
                        >
                          <Check className="h-4 w-4" />
                        </Button>
                      }
                    />
                  </form>
                  {returnScanAlert ? <DeskScanNotice alert={returnScanAlert} place="return" /> : null}
                  {returnResult?.kind === 'deskNotice' ? (
                    <div className="mt-3">
                      <ReturnDeskNotice
                        notice={returnResult.notice}
                        putBack={returnResult.putBack}
                        finding={lostFoundLookup}
                        onFound={
                          returnResult.barcode && returnNoticeOffersLostFound(returnResult.notice)
                            ? () => {
                                void openLostFoundByBarcode(returnResult.barcode ?? '');
                              }
                            : undefined
                        }
                      />
                    </div>
                  ) : null}
                  {returnResult?.kind === 'pick' && returnResult.deskNotice ? (
                    <div className="mt-3">
                      <ReturnDeskNotice notice={returnResult.deskNotice} />
                    </div>
                  ) : null}
                  {returnError && (
                    <div
                      role="alert"
                      className="mt-3 p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800"
                    >
                      <div className="flex items-start gap-3">
                        <AlertTriangle className="h-5 w-5 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" />
                        <p className="text-sm font-medium text-red-800 dark:text-red-200">{returnError}</p>
                      </div>
                    </div>
                  )}
                  {isProcessingReturn && (
                    <div className="mt-4 flex items-center gap-3 text-sm text-gray-600 dark:text-gray-300">
                      <div className="h-5 w-5 border-2 border-emerald-600 border-t-transparent rounded-full animate-spin shrink-0" />
                      <span>{t('common.loading')}</span>
                    </div>
                  )}
                </div>
              </div>
            </div>
          </Card>

          {/* Step 2 — Last return recap */}
          <Card>
            <div className="flex flex-col gap-6">
              <div className="flex gap-4">
                <div
                  className="hidden sm:flex h-11 w-11 shrink-0 items-center justify-center rounded-2xl bg-gray-200 text-base font-bold text-gray-700 dark:bg-gray-700 dark:text-gray-200"
                  aria-hidden
                >
                  2
                </div>
                <div className="min-w-0 flex-1 pt-0.5">
                  <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
                    {t('loans.returnSummaryTitle')}
                  </h3>
                </div>
              </div>

              {returnToLenderMessage ? <ReturnToLenderNotice message={returnToLenderMessage} /> : null}

              {!returnResult && (
                <div className="rounded-xl border border-dashed border-gray-200 bg-gray-50/80 py-14 text-center dark:border-gray-700 dark:bg-gray-800/30">
                  <BookOpen className="mx-auto h-12 w-12 text-gray-300 dark:text-gray-600 mb-3" />
                  <p className="text-sm text-gray-500 dark:text-gray-400 max-w-sm mx-auto px-4">
                    {t('loans.returnSummaryEmpty')}
                  </p>
                </div>
              )}

              {returnResult?.kind === 'lostFound' && (
                <LostFoundSummaryPanel summary={returnResult} locale={i18n.language} t={t} />
              )}
              {returnResult?.kind === 'loan' && (
                <ReturnRecapPanel
                  loan={returnResult.loan}
                  locale={i18n.language}
                  t={t}
                  canReadBorrowers={canReadBorrowers}
                />
              )}
              {returnResult?.kind === 'deskNotice' && (
                <ReturnDeskNotice
                  notice={returnResult.notice}
                  putBack={returnResult.putBack}
                  finding={lostFoundLookup}
                  onFound={
                    returnResult.barcode && returnNoticeOffersLostFound(returnResult.notice)
                      ? () => {
                          void openLostFoundByBarcode(returnResult.barcode ?? '');
                        }
                      : undefined
                  }
                />
              )}
              {returnResult?.kind === 'pick' && returnResult.deskNotice ? (
                <ReturnDeskNotice notice={returnResult.deskNotice} />
              ) : null}
              {returnResult?.kind === 'pick' && (
                <PickCheckInBanner notice={returnResult.notice} t={t} />
              )}
              {returnResult?.kind === 'item' && (
                <div className="rounded-xl border border-gray-200 bg-white p-5 dark:border-gray-800 dark:bg-gray-900">
                  <p className="text-base font-semibold text-gray-900 dark:text-white">
                    {returnResult.title?.trim() || t('loans.noTitle')}
                  </p>
                  <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
                    {t('items.barcode')}: <span className="font-mono">{returnResult.barcode ?? '-'}</span>
                  </p>
                  <div className="mt-3">
                    <ItemAvailabilityBadge availability={returnResult.availability} audience="staff" />
                  </div>
                </div>
              )}
            </div>
          </Card>
        </div>
      )}

      {activeTab === 'overdue' && (
        <div className="space-y-4">
          <Card padding="none" className="overflow-hidden">
            <div className="p-4 sm:p-5 border-b border-gray-100 dark:border-gray-800 bg-gradient-to-r from-rose-50/80 via-white to-white dark:from-rose-950/20 dark:via-gray-900 dark:to-gray-900">
              <CardHeader
                title={t('loans.overdueTab')}
                action={
                  userIsAdmin ? (
                    <div className="flex flex-wrap gap-2">
                      <Button
                        size="sm"
                        variant="secondary"
                        leftIcon={<FlaskConical className="h-4 w-4" />}
                        isLoading={reminderSending}
                        onClick={() => void handleSendReminders(true)}
                      >
                        {t('loans.remindersDryRun')}
                      </Button>
                      <Button
                        size="sm"
                        variant="primary"
                        leftIcon={<Send className="h-4 w-4" />}
                        isLoading={reminderSending}
                        onClick={() => void handleSendReminders(false)}
                      >
                        {t('loans.sendOverdueReminders')}
                      </Button>
                    </div>
                  ) : null
                }
              />
              {!userIsAdmin && (
                <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">{t('loans.remindersAdminOnly')}</p>
              )}
              {overdueError && (
                <RenewalRefusalNotice
                  part="alert"
                  view={{ kind: 'sentence', message: overdueError }}
                  onForce={() => undefined}
                  onDismiss={() => undefined}
                  onSetAside={() => undefined}
                />
              )}
            </div>

            {reminderReport && (
              <div className="mx-4 mt-4 rounded-2xl border border-indigo-200/80 dark:border-indigo-800 bg-indigo-50/90 dark:bg-indigo-950/30 p-4 text-sm space-y-3">
                <p className="font-semibold text-gray-900 dark:text-white">
                  {reminderReport.dryRun ? t('loans.reminderDryRunResult') : t('loans.reminderSendResult')}
                </p>
                <p className="text-gray-700 dark:text-gray-300">
                  {t('loans.reminderReportEmails', { count: reminderReport.emailsSent })} ·{' '}
                  {t('loans.reminderReportLoans', { count: reminderReport.loansReminded })}
                </p>
                {reminderReport.details.length > 0 && (
                  <div className="flex flex-wrap gap-2 max-h-36 overflow-y-auto pr-1">
                    {reminderReport.details.map((d) => (
                      <span
                        key={d.userId}
                        className="inline-flex items-center gap-1.5 rounded-lg bg-white/90 dark:bg-gray-900/60 border border-indigo-100 dark:border-indigo-900 px-2.5 py-1 text-xs text-gray-700 dark:text-gray-300"
                      >
                        <span className="font-medium">
                          {canReadBorrowers
                            ? [d.firstname, d.lastname].filter(Boolean).join(' ').trim() || '—'
                            : '—'}
                        </span>
                        {canReadBorrowers ? (
                          <>
                            <span className="text-gray-400">·</span>
                            <span className="truncate max-w-[10rem]">{d.email}</span>
                          </>
                        ) : null}
                        <Badge variant="default" size="sm">
                          {d.loanCount}
                        </Badge>
                      </span>
                    ))}
                  </div>
                )}
                {reminderReport.errors.length > 0 && (
                  <ul className="text-red-700 dark:text-red-400 text-xs space-y-1 border-t border-red-200/50 dark:border-red-900/40 pt-2">
                    {reminderReport.errors.map((e) => (
                      <li key={`${e.userId}-${e.email}`}>
                        {canReadBorrowers ? `${e.email}: ` : null}
                        {e.errorMessage}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}

            <div className="p-4 sm:p-5 space-y-4">
              {overdueLoading ? (
                <div className="flex justify-center py-16">
                  <div className="h-9 w-9 border-2 border-rose-500 border-t-transparent rounded-full animate-spin" />
                </div>
              ) : overdueByUser.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-14 text-center px-4">
                  <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-emerald-50 dark:bg-emerald-950/40 mb-4">
                    <Bell className="h-7 w-7 text-emerald-600 dark:text-emerald-400 opacity-80" />
                  </div>
                  <p className="text-sm font-medium text-gray-900 dark:text-white">{t('loans.noOverdueLoans')}</p>
                </div>
              ) : (
                <div className="space-y-2.5">
                  {overdueByUser.map(([uid, userLoans]) => {
                    const u = userLoans[0];
                    const lastAny = maxReminderSentAt(userLoans);
                    const locale = i18n.language;
                    return (
                      <details
                        key={uid}
                        className="group rounded-2xl border border-gray-200/90 dark:border-gray-700/90 bg-white dark:bg-gray-900/50 shadow-sm overflow-hidden transition-shadow hover:shadow-md open:shadow-md"
                        open={overdueOpenByUser[uid] ?? false}
                        onToggle={(e) => {
                          const el = e.currentTarget;
                          setOverdueOpenByUser((p) => ({ ...p, [uid]: el.open }));
                        }}
                      >
                        <summary className="cursor-pointer list-none [&::-webkit-details-marker]:hidden flex items-center gap-3 px-3 py-3 sm:px-4 sm:py-3.5 hover:bg-rose-50/50 dark:hover:bg-rose-950/15 open:bg-rose-50/60 dark:open:bg-rose-950/20 transition-colors">
                          <ChevronDown className="h-4 w-4 shrink-0 text-gray-400 transition-transform duration-200 group-open:rotate-180" aria-hidden />
                          <OverdueBorrowerSummary
                            user={u.user}
                            count={userLoans.length}
                            linked={canReadBorrowers}
                          />
                          <div className="hidden sm:flex shrink-0 flex-col items-end gap-0.5 text-right text-[11px] text-gray-400 dark:text-gray-500">
                            <span className="whitespace-nowrap uppercase tracking-wide">
                              {t('loans.lastReminderAny')}
                            </span>
                            <span className="tabular-nums text-gray-600 dark:text-gray-300">
                              {lastAny
                                ? new Date(lastAny).toLocaleString(locale, {
                                    dateStyle: 'short',
                                    timeStyle: 'short',
                                  })
                                : t('loans.neverReminded')}
                            </span>
                          </div>
                        </summary>
                        <div className="border-t border-gray-100 dark:border-gray-800 bg-gray-50/60 dark:bg-gray-950/40">
                          {userLoans.map((row) => {
                            const late = daysPastDue(row.expiryAt);
                            const claimed = claimedLoanIds.has(row.loanId);
                            const nextTier = nextReminderTierFromCount(row.reminderCount);
                            const isReturnLoading =
                              overdueLoanAction?.loanId === row.loanId && overdueLoanAction.op === 'return';
                            const isRenewLoading =
                              overdueLoanAction?.loanId === row.loanId && overdueLoanAction.op === 'renew';
                            const overdueBusyKind =
                              overdueLoanAction?.loanId === row.loanId &&
                              overdueLoanAction.op !== 'return' &&
                              overdueLoanAction.op !== 'renew'
                                ? overdueLoanAction.op
                                : null;
                            return (
                              <div
                                key={row.loanId}
                                className="border-b border-gray-100/90 dark:border-gray-800/80 px-3 py-3 sm:px-4 last:border-b-0"
                              >
                                <div className="grid grid-cols-1 sm:grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-2">
                                  <p className="text-sm font-semibold leading-snug text-gray-900 dark:text-white sm:col-start-1 sm:row-start-1 line-clamp-2">
                                    {row.title || t('loans.noTitle')}
                                  </p>
                                  <p className="text-xs text-gray-500 dark:text-gray-400 sm:col-start-1 sm:row-start-2 line-clamp-2 min-h-[1.25rem]">
                                    {row.authors ?? '\u00a0'}
                                  </p>
                                  <div className={`${OVERDUE_FACTS_CLASS} sm:col-start-2 sm:row-start-1`}>
                                    <code className="shrink-0 whitespace-nowrap rounded bg-white/90 dark:bg-gray-900/80 px-2 py-0.5 font-mono text-[11px] text-gray-700 dark:text-gray-300 border border-gray-200/80 dark:border-gray-700">
                                      {row.itemBarcode ?? '—'}
                                    </code>
                                    <OverdueDueDate iso={row.expiryAt} locale={locale} />
                                    {late > 0 ? (
                                      <span className="shrink-0 rounded-md bg-amber-100 px-1.5 py-0.5 text-[10px] font-semibold tabular-nums tracking-wide text-amber-900 dark:bg-amber-900/40 dark:text-amber-100">
                                        {t('loans.overdueDaysLate', { count: late })}
                                      </span>
                                    ) : null}
                                    <span
                                      className="inline-flex shrink-0 items-center gap-1 text-gray-500 dark:text-gray-400 sm:ml-0.5"
                                      title={
                                        row.lastReminderSentAt
                                          ? new Date(row.lastReminderSentAt).toLocaleString(locale)
                                          : t('loans.neverReminded')
                                      }
                                    >
                                      <Bell className="h-3.5 w-3.5 shrink-0 opacity-70" />
                                      <span>
                                        {nextTier
                                          ? t(`loans.nextReminderTier.${nextTier}`)
                                          : t('loans.nextReminderTier.none')}
                                      </span>
                                    </span>
                                  </div>
                                  <div className="flex flex-col gap-2 sm:col-start-2 sm:row-start-2">
                                    <div className="flex flex-row gap-2">
                                      {!claimed && (
                                        <Button
                                          type="button"
                                          size="sm"
                                          variant="ghost"
                                          className="min-w-0 flex-1 justify-center"
                                          isLoading={isRenewLoading}
                                          disabled={overdueDeskBusy}
                                          leftIcon={<RotateCcw className="h-4 w-4" />}
                                          onClick={(e) => {
                                            e.stopPropagation();
                                            void handleOverdueRenew(row.loanId);
                                          }}
                                        >
                                          {t('loans.renew')}
                                        </Button>
                                      )}
                                      <Button
                                        type="button"
                                        size="sm"
                                        variant="primary"
                                        className="min-w-0 flex-1 justify-center"
                                        isLoading={isReturnLoading}
                                        disabled={overdueDeskBusy}
                                        leftIcon={<Check className="h-4 w-4" />}
                                        onClick={(e) => {
                                          e.stopPropagation();
                                          void handleOverdueReturn(row.loanId);
                                        }}
                                      >
                                        {t('loans.return')}
                                      </Button>
                                    </div>
                                    <LoanExceptionActions
                                      loanId={row.loanId}
                                      loanLabel={row.title || undefined}
                                      claimedReturned={claimed}
                                      disabled={overdueDeskBusy}
                                      busyKind={overdueBusyKind}
                                      onOpen={(kind, label) => exceptionAction.open({ loanId: row.loanId, kind, label })}
                                    />
                                  </div>
                                </div>
                              </div>
                            );
                          })}
                        </div>
                      </details>
                    );
                  })}
                </div>
              )}
              {overdueData && overdueData.total > 0 && (
                <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 rounded-2xl bg-gray-50 dark:bg-gray-800/50 px-4 py-3 border border-gray-100 dark:border-gray-800">
                  <span className="text-sm text-gray-600 dark:text-gray-400">
                    {t('loans.overdueTotal', { total: overdueData.total })}
                  </span>
                  <div className="flex flex-wrap items-center gap-3">
                    <label className={formChoiceLabelClass()}>
                      {t('common.perPage')}
                      <select
                        value={overduePerPage}
                        onChange={(e) => {
                          setOverduePerPage(Number(e.target.value));
                          setOverduePage(1);
                        }}
                        className={formControlClass()}
                      >
                        {[25, 50, 100, 200].map((n) => (
                          <option key={n} value={n}>
                            {n}
                          </option>
                        ))}
                      </select>
                    </label>
                    <Pagination
                      currentPage={overduePage}
                      totalPages={overdueTotalPages}
                      onPageChange={setOverduePage}
                    />
                  </div>
                </div>
              )}
            </div>
          </Card>
        </div>
      )}

      {activeTab === 'claims' && (
        <ClaimsReturnedQueue
          linkReaders={canReadBorrowers}
          disabled={exceptionAction.isLoading || exceptionOpen || borrowLoanAction != null}
          busyLoanId={exceptionAction.target?.loanId ?? (borrowLoanAction?.loanId ?? overdueLoanAction?.loanId ?? null)}
          busyKind={
            exceptionAction.target?.kind ??
            (borrowLoanAction && borrowLoanAction.op !== 'return' && borrowLoanAction.op !== 'renew'
              ? borrowLoanAction.op
              : overdueLoanAction && overdueLoanAction.op !== 'return' && overdueLoanAction.op !== 'renew'
                ? overdueLoanAction.op
                : null)
          }
          onOpen={(loanId, kind, label) => exceptionAction.open({ loanId, kind, label })}
        />
      )}

      <CirculationExceptionDialog
        target={exceptionAction.target}
        isLoading={exceptionAction.isLoading}
        error={exceptionAction.error}
        replacementAmountRequired={exceptionAction.replacementAmountRequired}
        onClose={exceptionAction.close}
        onConfirm={(payload) => void exceptionAction.submit(payload)}
      />

      <MessageModal
        isOpen={messageDialog !== null}
        onClose={() => {
          setMessageDialog(null);
          focusScanInput();
        }}
        message={messageDialog ?? ''}
      />

      <ConfirmDialog
        isOpen={sendRemindersConfirmOpen}
        onClose={() => {
          setSendRemindersConfirmOpen(false);
          focusScanInput();
        }}
        onConfirm={() => {
          setSendRemindersConfirmOpen(false);
          void executeSendReminders(false);
          focusScanInput();
        }}
        message={t('loans.sendRemindersConfirm')}
        confirmVariant="primary"
      />

      <ForceCirculationDialog
        isOpen={forceBorrowDialog !== null}
        message={forceBorrowDialog?.message ?? ''}
        confirmLabel={t('loans.forceLoan')}
        reasonRequired={forceBorrowDialog?.reasonRequired === true}
        error={forceBorrowDialog?.error}
        isLoading={forceBorrowLoading}
        onCancel={() => {
          if (forceBorrowLoading) return;
          setForceBorrowDialog(null);
          focusScanInput();
        }}
        onConfirm={(reason) => {
          const ctx = forceBorrowDialog;
          if (!ctx || forceBorrowLoading) return;
          setForceBorrowLoading(true);
          void (async () => {
            try {
              await handleBorrow(ctx.barcode, true, reason);
              setForceBorrowDialog(null);
              setBarcodeInput('');
              focusScanInput();
            } catch (error: unknown) {
              const held = overrideReasonRefusalMessage(error);
              if (held !== null) {
                setForceBorrowDialog((current) => (current ? { ...current, error: held } : current));
                return;
              }
              setForceBorrowDialog(null);
              const msg = deskBorrowErrorText(error, t);
              if (msg) showToast({ variant: 'error', message: msg });
              focusScanInput();
            } finally {
              setForceBorrowLoading(false);
            }
          })();
        }}
      />

      <ToPickForceLoanDialog
        isOpen={toPickAside !== null}
        message={toPickAside?.message}
        reasonRequired={toPickAside?.reasonRequired === true}
        error={toPickAside?.error}
        isLoading={toPickAsideLoading}
        onForce={
          toPickAside?.force
            ? (reason) => {
                const barcode = toPickAside.barcode;
                if (!barcode || toPickAsideLoading) return;
                setToPickAsideLoading(true);
                void (async () => {
                  try {
                    await handleBorrow(barcode, true, reason);
                    setToPickAside(null);
                    setBarcodeInput('');
                    focusScanInput();
                  } catch (error: unknown) {
                    const held = overrideReasonRefusalMessage(error);
                    if (held !== null) {
                      setToPickAside((current) => (current ? { ...current, error: held } : current));
                      return;
                    }
                    if (!isCheckoutDeclined(error)) setToPickAside(null);
                    const msg = deskBorrowErrorText(error, t);
                    if (msg) showToast({ variant: 'error', message: msg });
                    if (!isCheckoutDeclined(error)) focusScanInput();
                  } finally {
                    setToPickAsideLoading(false);
                  }
                })();
              }
            : undefined
        }
        onCancel={() => {
          if (toPickAsideLoading) return;
          commitDismissHeldSetAside({
            closeDialog: () => setToPickAside(null),
            clearBarcode: () => setBarcodeInput(''),
            refocusBarcode: () => focusScanInput(),
          });
        }}
        onSetAside={() => {
          const current = toPickAside;
          if (!current || toPickAsideLoading) return;
          setToPickAsideLoading(true);
          void commitHeldSetAside(current.barcode, {
            holdId: current.holdId,
            itemId: current.itemId,
            resolveItemId: copyIdForBarcode,
            returnSetAside: (code, { holdId, itemId }) => api.returnLoanByBarcode(code, { holdId, itemId }),
            successMessage: (result) => heldSetAsideSuccessMessage(result, t),
            errorMessage: (error) => heldSetAsideServerMessage(error, t),
            showToast: (toast) => showToast(toast),
            closeDialog: () => setToPickAside(null),
            clearBarcode: () => setBarcodeInput(''),
            refocusBarcode: () => focusScanInput(),
          }).finally(() => setToPickAsideLoading(false));
        }}
      />

      <RenewalRefusalNotice
        part="dialog"
        view={renewalPrompt}
        busy={renewalAsideLoading || borrowLoanAction?.op === 'renew' || overdueLoanAction?.op === 'renew'}
        onForce={(reason) => {
          renewalRetryRef.current?.(reason);
        }}
        onDismiss={() => {
          if (renewalAsideLoading) return;
          renewalRetryRef.current = null;
          setRenewalPrompt(null);
          focusScanInput();
        }}
        onSetAside={() => {
          const target = renewalSetAsideRef.current;
          if (!target || !target.barcode || renewalAsideLoading) return;
          setRenewalAsideLoading(true);
          void commitHeldSetAside(target.barcode, {
            holdId: target.holdId,
            itemId: target.itemId,
            resolveItemId: copyIdForBarcode,
            returnSetAside: (code, { holdId, itemId }) => api.returnLoanByBarcode(code, { holdId, itemId }),
            successMessage: (result) => heldSetAsideSuccessMessage(result, t),
            errorMessage: (error) => heldSetAsideServerMessage(error, t),
            showToast: (toast) => showToast(toast),
            closeDialog: () => {
              setRenewalPrompt(null);
              renewalRetryRef.current = null;
              if (selectedUser) void applyPatronLoans({ notify: false });
            },
            clearBarcode: () => setBarcodeInput(''),
            refocusBarcode: () => focusScanInput(),
          }).finally(() => setRenewalAsideLoading(false));
        }}
      />

      <LostFoundAlertDialog
        isOpen={lostFound !== null}
        fee={lostFound ?? {}}
        canResolve={Boolean(lostFound?.itemId)}
        pending={lostFoundPending}
        error={lostFoundError}
        onDismiss={dismissLostFound}
        onResolve={(disposition) => {
          void resolveLostFoundChoice(disposition);
        }}
      />

    </div>
  );
}

async function readDeskCopy(barcode: string): Promise<DeskCopySnapshot | null> {
  try {
    const notice = await api.getItemByBarcode(barcode);
    const copies = notice.items ?? [];
    const copy =
      copies.find((item) => (item.barcode ?? '').trim() === barcode) ?? copies[0];
    if (!copy || !isItemStaffView(copy)) return null;
    let loanUserId: string | null = null;
    let loanUserName: string | null = null;
    let loanUserCard: string | null = null;
    let loanUserInitials: string | null = null;
    let loanUserFirstname: string | null = null;
    let loanUserLastname: string | null = null;
    if (copy.availability === 'onLoan' && copy.loanId) {
      try {
        const borrower = await api.getLoanBorrower(copy.loanId);
        const reveal = canReadUsers(null, api.getToken());
        const ref = otherBorrowerFromPerson({
          userId: borrower.id,
          cardNumber: borrower.cardNumber,
          barcode: borrower.barcode,
          initials: borrower.initials,
          firstname: reveal ? borrower.firstname : null,
          lastname: reveal ? borrower.lastname : null,
        });
        loanUserId = ref?.userId ?? borrower.id;
        loanUserName = reveal ? readerName(borrower) : null;
        loanUserCard = ref?.cardNumber ?? null;
        loanUserInitials = ref?.initials ?? null;
        loanUserFirstname = reveal ? (borrower.firstname ?? null) : null;
        loanUserLastname = reveal ? (borrower.lastname ?? null) : null;
      } catch {
        loanUserId = null;
        loanUserName = null;
      }
    }
    return {
      itemId: copy.id,
      availability: copy.availability,
      loanId: copy.loanId ?? null,
      loanUserId,
      loanUserName,
      loanUserCard,
      loanUserInitials,
      loanUserFirstname,
      loanUserLastname,
      holdFor: copy.holdFor ?? null,
      lenderName: copy.deposit?.lenderName?.trim() || null,
    };
  } catch {
    return null;
  }
}

async function resolvePickNotice(notice: PickCheckInNotice): Promise<PickCheckInNotice> {
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
      const sources = await api.getSources(false);
      siteName = sourceLabel(sources, notice.siteId);
    } catch {
      siteName = null;
    }
  }
  return { ...notice, patronName, siteName };
}

function PickCheckInBanner({ notice, t }: { notice: PickCheckInNotice; t: TFunction }) {
  const ready = notice.kind === 'ready';
  const message = ready
    ? t('loans.pickReady', { patron: notice.patronName?.trim() || t('loans.pickPatronUnknown') })
    : t('loans.pickTransit', { site: notice.siteName?.trim() || t('loans.pickSiteUnknown') });
  return (
    <div
      role="status"
      className={`rounded-xl border px-4 py-4 text-base font-semibold ${
        ready
          ? 'border-emerald-200 bg-emerald-50 text-emerald-950 dark:border-emerald-900/50 dark:bg-emerald-950/30 dark:text-emerald-100'
          : 'border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900/50 dark:bg-amber-950/30 dark:text-amber-100'
      }`}
    >
      {message}
    </div>
  );
}

function CheckoutRecapPanel({
  recap,
  locale,
  t,
}: {
  recap: CheckoutRecap;
  locale: string;
  t: TFunction;
}) {
  const df = (iso: string) =>
    new Date(iso).toLocaleDateString(locale, {
      day: 'numeric',
      month: 'short',
      year: 'numeric',
    });

  const initials = recap.patronName
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0])
    .join('')
    .toUpperCase() || '?';

  return (
    <div className="overflow-hidden rounded-xl border border-amber-200 bg-white shadow-sm dark:border-amber-900/50 dark:bg-gray-900/40">
      <div className="flex items-center gap-2.5 border-b border-amber-200/80 bg-amber-50 px-4 py-2 dark:border-amber-900/50 dark:bg-amber-950/25">
        <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-amber-100 dark:bg-amber-900/50">
          <BookMarked className="h-3.5 w-3.5 text-amber-700 dark:text-amber-400" aria-hidden />
        </div>
        <p className="min-w-0 text-sm font-medium leading-snug text-amber-900 dark:text-amber-100">
          <span className="font-semibold">{t('loans.checkoutSuccessShort')}</span>
          <span className="font-normal text-amber-800/85 dark:text-amber-200/90">
            {' '}
            · {t('loans.checkoutProcessed')}
          </span>
        </p>
      </div>
      <div className="px-4 py-3 space-y-2">
        <p className="text-sm font-semibold leading-snug text-gray-900 dark:text-white">
          {recap.title || t('loans.noTitle')}
        </p>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-gray-600 dark:text-gray-300">
          <span>
            <span className="text-gray-400 dark:text-gray-500 uppercase tracking-wide mr-1">
              {t('loans.specimenBarcode')}
            </span>
            <span className="font-mono">{recap.barcode || '—'}</span>
          </span>
          <span>
            <span className="text-gray-400 dark:text-gray-500 uppercase tracking-wide mr-1">
              {t('loans.dueDate')}
            </span>
            <span className="font-semibold tabular-nums text-gray-900 dark:text-white">
              {df(recap.dueDate)}
            </span>
          </span>
        </div>
        {recap.dueDateCappedByDeposit ? (
          <p className="text-xs text-gray-700 dark:text-gray-200">
            {depositDueDateNotice(
              { dueDateCappedByDeposit: true, expiryAt: recap.dueDate },
              t,
              locale,
            )}
          </p>
        ) : null}
        <div className="flex items-center gap-2 pt-1">
          <div
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-indigo-100 text-[10px] font-bold uppercase text-indigo-800 dark:bg-indigo-900/55 dark:text-indigo-200"
            aria-hidden
          >
            {initials}
          </div>
          <p className="min-w-0 truncate text-sm font-medium text-gray-900 dark:text-white">
            {recap.patronName || '—'}
          </p>
        </div>
      </div>
    </div>
  );
}

function LostFoundSummaryPanel({
  summary,
  locale,
  t,
}: {
  summary: LostFoundSummary;
  locale: string;
  t: TFunction;
}) {
  const found = new Date(summary.foundAt);
  const foundLabel = Number.isNaN(found.getTime())
    ? summary.foundAt
    : found.toLocaleString(locale, { dateStyle: 'medium', timeStyle: 'short' });

  return (
    <div className="overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-900/40">
      <div className="px-5 py-5">
        <p className="text-lg font-semibold leading-snug text-gray-900 dark:text-white">
          {summary.title?.trim() || t('loans.noTitle')}
        </p>
        {summary.barcode ? (
          <p className="mt-2 text-sm text-gray-600 dark:text-gray-300">
            {t('items.barcode')}: <span className="font-mono">{summary.barcode}</span>
          </p>
        ) : null}
        {summary.patronName ? (
          <p className="mt-1 text-sm text-gray-600 dark:text-gray-300">
            {t('loans.exceptions.patron')}: {summary.patronName}
          </p>
        ) : null}
      </div>
      <dl className="grid gap-5 border-t border-gray-100 px-5 py-4 sm:grid-cols-3 dark:border-gray-800">
        <div>
          <dt className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
            {t('loans.lostFound.summaryFoundAt')}
          </dt>
          <dd className="mt-1 text-sm font-semibold text-gray-900 dark:text-white">{foundLabel}</dd>
        </div>
        <div>
          <dt className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
            {t('loans.lostFound.summaryAction')}
          </dt>
          <dd className="mt-1 text-sm font-semibold text-gray-900 dark:text-white">
            {lostFoundActionLabel(summary.action, (key, values) => String(t(key, values)))}
          </dd>
        </div>
        <div>
          <dt className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
            {t('loans.lostFound.summaryFees')}
          </dt>
          <dd className="mt-1 text-sm font-semibold text-gray-900 dark:text-white">{summary.feeText}</dd>
        </div>
      </dl>
    </div>
  );
}

function ReturnRecapPanel({
  loan,
  locale,
  t,
  canReadBorrowers,
}: {
  loan: Loan;
  locale: string;
  t: TFunction;
  canReadBorrowers: boolean;
}) {
  const biblio = loan.biblio;
  const authorLine = biblio.author
    ? [biblio.author.firstname, biblio.author.lastname].filter(Boolean).join(' ').trim()
    : '';
  const df = (iso: string) =>
    new Date(iso).toLocaleDateString(locale, {
      day: 'numeric',
      month: 'short',
      year: 'numeric',
    });

  const barcode = loan.itemIdentification?.trim() || '—';

  return (
    <div className="sm:ml-2 sm:border-l-2 sm:border-gray-200 sm:pl-6 sm:dark:border-gray-700">
      <div className="overflow-hidden rounded-2xl border border-gray-200 bg-white shadow-sm dark:border-gray-700 dark:bg-gray-900/40">
        <div className="flex items-center gap-2.5 border-b border-emerald-200/80 bg-emerald-50 px-4 py-2.5 dark:border-emerald-900/50 dark:bg-emerald-950/25">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-emerald-100 dark:bg-emerald-900/50">
            <Check className="h-4 w-4 text-emerald-700 dark:text-emerald-400" aria-hidden />
          </div>
          <p className="min-w-0 text-sm font-medium leading-snug text-emerald-900 dark:text-emerald-100">
            <span className="font-semibold">{t('loans.returnSuccess')}</span>
            <span className="font-normal text-emerald-800/85 dark:text-emerald-200/90"> · {t('loans.returnProcessed')}</span>
          </p>
        </div>

        <div className="divide-y divide-gray-100 dark:divide-gray-800">
          <div className="px-5 py-5">
            <p className="text-lg font-semibold leading-snug text-gray-900 dark:text-white">
              {biblio.title || t('loans.noTitle')}
            </p>
            <p className="mt-2 text-sm text-gray-600 dark:text-gray-300">{authorLine || '—'}</p>
            <p className="mt-4 text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
              {t('loans.specimenBarcode')}
            </p>
            <p className="mt-1 font-mono text-sm tracking-wide text-gray-800 dark:text-gray-200">{barcode}</p>
          </div>

          <div className="grid gap-5 px-5 py-4 sm:grid-cols-3 sm:gap-6">
            <div>
              <p className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
                {t('loans.borrowDate')}
              </p>
              <p className="mt-1 text-sm font-semibold text-gray-900 dark:text-white tabular-nums">{df(loan.startDate)}</p>
            </div>
            <div>
              <p className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
                {t('loans.dueDate')}
              </p>
              <p className="mt-1 text-sm font-semibold text-gray-900 dark:text-white tabular-nums">{df(loan.expiryAt)}</p>
            </div>
            <div>
              <p className="text-xs font-medium uppercase tracking-wide text-gray-400 dark:text-gray-500">
                {t('loans.renewals')}
              </p>
              <p className="mt-1 text-sm font-semibold text-gray-900 tabular-nums dark:text-white">{loan.nbRenews}</p>
            </div>
          </div>

          <div className="bg-gray-50/90 px-5 py-4 dark:bg-gray-800/35">
            <ReturnBorrowerLine user={loan.user} linked={canReadBorrowers} />
          </div>
        </div>
      </div>
    </div>
  );
}

interface BorrowFormProps {
  formId: string;
  onBorrow: (barcode: string, force?: boolean) => Promise<void>;
  barcodeInput: string;
  setBarcodeInput: (value: string) => void;
  barcodeInputRef: React.RefObject<HTMLInputElement | null>;
  onLoadingChange: (loading: boolean) => void;
  onSuccess?: () => void;
  variant?: 'default' | 'inline';
  isLoading?: boolean;
}

function BorrowForm({
  formId,
  onBorrow,
  barcodeInput,
  setBarcodeInput,
  barcodeInputRef,
  onLoadingChange,
  onSuccess,
  variant = 'default',
  isLoading = false,
}: BorrowFormProps) {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const [error, setError] = useState('');
  const borrowInFlightRef = useRef(false);

  const runBorrow = async (overrideBarcode?: string) => {
    const code = (overrideBarcode ?? barcodeInput).trim();
    if (!code || isLoading || borrowInFlightRef.current) return;

    borrowInFlightRef.current = true;
    setError('');
    onLoadingChange(true);
    try {
      await onBorrow(code);
      setBarcodeInput('');
      setError('');
      if (onSuccess) {
        onSuccess();
      } else {
        requestAnimationFrame(() => {
          barcodeInputRef.current?.focus();
        });
      }
    } catch (err: unknown) {
      const msg = deskBorrowErrorText(err, t);
      if (msg == null) {
        if (isCheckoutScanNoticeShown(err)) {
          setError('');
          setTimeout(() => {
            barcodeInputRef.current?.focus();
            barcodeInputRef.current?.select();
          }, 100);
        }
        return;
      }
      setError(msg);
      showToast({ variant: 'error', message: msg });
      setTimeout(() => {
        barcodeInputRef.current?.focus();
        barcodeInputRef.current?.select();
      }, 100);
    } finally {
      borrowInFlightRef.current = false;
      onLoadingChange(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    await runBorrow();
  };

  const isInline = variant === 'inline';

  return (
    <form id={formId} onSubmit={handleSubmit} className={isInline ? 'space-y-2' : 'space-y-4'}>
      <BarcodeScanField
        ref={barcodeInputRef}
        label={isInline ? undefined : t('loans.specimenBarcode')}
        value={barcodeInput}
        onChange={(e) => {
          setBarcodeInput(e.target.value);
          if (error) setError('');
        }}
        placeholder={t('loans.scanOrEnterBarcode')}
        autoFocus={isInline}
        required={!isInline}
        disabled={isLoading}
        leftIcon={<BookOpen className="h-4 w-4" />}
        scannerTitle={t('loans.scanSpecimenBarcode')}
        onCameraScan={(barcode) => {
          setBarcodeInput(barcode);
          if (error) setError('');
          void runBorrow(barcode);
        }}
        rightIcon={
          isInline ? (
            <Button
              type="submit"
              size="icon"
              variant="primary"
              disabled={isLoading || !barcodeInput.trim()}
              aria-label={t('common.validate')}
            >
              <Check className="h-4 w-4" />
            </Button>
          ) : undefined
        }
      />
      {error && (
        <div
          role="alert"
          className="p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800"
        >
          <p className="text-sm font-medium text-red-800 dark:text-red-200">{error}</p>
        </div>
      )}
    </form>
  );
}
