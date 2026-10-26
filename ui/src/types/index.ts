import {
  readAcquisitionsRightsFromJwt,
  readHoldsRightsFromJwt,
  readItemsArchiveRightsFromJwt,
  readItemsRightsFromJwt,
  readOwnableRightFromJwt,
  normalizeUsersRightsLevel,
  readLoansRightsFromJwt,
  readUsersRightsFromJwt,
  readWeedingCancelRightsFromJwt,
  readWeedingRightsFromJwt,
} from '@/utils/jwtRights';

/** Instance module flags from `GET /library-info` (`enabledFeatures`, camelCase). Missing = enabled. */
export interface EnabledFeatures {
  acquisitions: boolean;
  purchaseSuggestions: boolean;
  weeding: boolean;
  transits: boolean;
  deposits: boolean;
  events: boolean;
  inventory: boolean;
  z3950: boolean;
  importIso: boolean;
  stats: boolean;
  visitorCounts: boolean;
}

/** Admin `PUT /admin/config/features` body (snake_case on the wire, like reminders). */
export interface FeaturesConfig {
  overridable?: boolean;
  profile?: string | null;
  acquisitions: boolean;
  purchase_suggestions: boolean;
  weeding: boolean;
  transits: boolean;
  deposits: boolean;
  events: boolean;
  inventory: boolean;
  z3950: boolean;
  import_iso: boolean;
  stats: boolean;
  visitor_counts: boolean;
}

// Library info types
export interface LibraryInfo {
  name?: string | null;
  addrLine1?: string | null;
  addrLine2?: string | null;
  addrPostcode?: string | null;
  addrCity?: string | null;
  addrCountry?: string | null;
  email?: string | null;
  phones: string[];
  /** IANA time zone name (for example `Europe/Paris`). */
  timezone: string;
  /**
   * UI language from `GET /api/v1/library-info`: `fr`, `en`, `de`, `es`, or `null`.
   * Absent, null, or blank means the client falls back to French.
   * Optional so this client still typechecks before the server field exists.
   */
  language?: string | null;
  updatedAt?: string | null;
  /** Optional modules for this instance. Absent keys are treated as enabled. */
  enabledFeatures?: Partial<EnabledFeatures> | null;
}

export interface UpdateLibraryInfoRequest {
  name?: string | null;
  addrLine1?: string | null;
  addrLine2?: string | null;
  addrPostcode?: string | null;
  addrCity?: string | null;
  addrCountry?: string | null;
  email?: string | null;
  phones?: string[] | null;
  /** IANA time zone name. Omitted values are left unchanged. */
  timezone?: string | null;
  /**
   * Default public UI language. Omit the field to leave the stored value unchanged.
   * `null` clears it. Allowed values are exactly `fr`, `en`, `de`, and `es`.
   * Any other value is HTTP 400 `validation_error`.
   * Optional so this client still typechecks before the server field exists.
   */
  language?: string | null;
}

// Schedule types
export interface SchedulePeriod {
  id: string;
  name: string;
  startDate: string;
  endDate: string;
  notes?: string | null;
  createdAt?: string | null;
  updateAt?: string | null;
}

export interface ScheduleSlot {
  id: string;
  periodId: string;
  dayOfWeek: number; // 0=Monday, 6=Sunday
  openTime: string;
  closeTime: string;
  createdAt?: string | null;
}

export interface ScheduleClosure {
  id: string;
  closureDate: string;
  reason?: string | null;
  createdAt?: string | null;
}

export interface CreateSchedulePeriod {
  name: string;
  startDate: string;
  endDate: string;
  notes?: string | null;
}

export interface UpdateSchedulePeriod {
  name?: string | null;
  startDate?: string | null;
  endDate?: string | null;
  notes?: string | null;
}

export interface CreateScheduleSlot {
  dayOfWeek: number;
  openTime: string;
  closeTime: string;
}

export interface CreateScheduleClosure {
  closureDate: string;
  reason?: string | null;
}

/** Nested permission bag on GET /auth/me (matches JWT `rights` when exposed). */
export interface AuthMeRights {
  holdsRights?: string | null;
  /** Legacy alias on some tokens — prefer holdsRights */
  borrowsRights?: string | null;
  /** Acquisitions (`n`/`r`/`w` or `none`/`read`/`write`) — distinct from cataloging. */
  acquisitionsRights?: string | null;
  /** Weeding list/export (`r`/`w`) and withdraw (`w`). Missing means `n`. */
  weedingRights?: string | null;
  /** Cancel an active withdrawal (`w`). Missing means `n`. */
  weedingCancelRights?: string | null;
  /** Delete a physical copy (`w`). Missing means `n`. */
  itemsArchiveRights?: string | null;
  /** Bibliographic record write, including `DELETE /biblios/{id}`. */
  itemsRights?: string | null;
  /** Borrower records (`n` / `o` / `r` / `w` or `none` / `own` / `read` / `write`). */
  usersRights?: string | null;
  /** Circulation (`n` / `o` / `r` / `w`). `own` is the patron's own loans. Write is required to send `force`. */
  loansRights?: string | null;
  /** Library settings (`n` / `r` / `w`). */
  settingsRights?: string | null;
  /** Cultural events (`n` / `r` / `w`). */
  eventsRights?: string | null;
}

// User types
export interface User {
  id: string;
  username: string;
  login?: string;
  firstname?: string;
  lastname?: string;
  email?: string;
  phone?: string;
  barcode?: string;
  accountType?: string;
  language?: string;
  /**
   * Holds permission from the API (`none` | `own` | `read` | `write`), when exposed.
   * Patron self-service uses `own`.
   */
  holdsRights?: string | null;
  /** Acquisitions permission when exposed on the profile (`n`/`r`/`w` or `none`/`read`/`write`). */
  acquisitionsRights?: string | null;
  weedingRights?: string | null;
  weedingCancelRights?: string | null;
  itemsArchiveRights?: string | null;
  itemsRights?: string | null;
  /** Borrower records when exposed on the profile. JWT `rights.usersRights` is preferred. */
  usersRights?: string | null;
  /** Circulation when exposed on the profile. JWT `rights.loansRights` is preferred. */
  loansRights?: string | null;
  /** Settings when exposed on the profile. JWT `rights.settingsRights` is preferred. */
  settingsRights?: string | null;
  /** Events when exposed on the profile. JWT `rights.eventsRights` is preferred. */
  eventsRights?: string | null;
  rights?: AuthMeRights | null;
  // Address fields
  addrStreet?: string;
  addrZipCode?: number;
  addrCity?: string;
  // Additional fields
  birthdate?: string;
  notes?: string;
  fee?: string;
  groupId?: string | null;
  /**
   * Audience. `GET /auth/me` and login `user` send the public-type **name**
   * (`child`, `adult`, …) or null. `GET /users` sends the public-type id.
   */
  publicType?: string | null;
  /**
   * True when this reader has no birth date. The stored public type is kept.
   * Absent or false for every other account, and for a reader who has a birth date.
   */
  birthDateMissing?: boolean;
  status?: number;
  // Date fields
  createdAt?: string;
  updateAt?: string;
  archivedAt?: string;
  /** Subscription / membership expiry (ISO). Null = unlimited. */
  expiryAt?: string | null;
  // 2FA fields
  twoFactorEnabled?: boolean;
  twoFactorMethod?: string | null;
  // Other
  receiveReminders?: boolean;
  mustChangePassword?: boolean;
  sex?: string | null;
  staffType?: string | null;
  hoursPerWeek?: number | null;
  staffStartDate?: string | null;
  staffEndDate?: string | null;
  /**
   * Linked legal guardian patron id. Required when public type is `child`.
   * Present on `GET /auth/me` (nullable snowflake string).
   */
  guardianId?: string | null;
  /**
   * Opt-in for the events programme. CamelCase JSON (`eventsConsent`).
   * Absent on child accounts: the guardian's consent applies.
   */
  eventsConsent?: boolean;
  /** When the current consent value was recorded. Null when never set. */
  eventsConsentAt?: string | null;
  /**
   * Who recorded the consent. The client never sends this; the server derives it.
   * `registration` | `desk` | `profile` | `migration` | `unsubscribe`.
   */
  eventsConsentSource?: EventsConsentSource | null;
}

/** Server values for `eventsConsentSource` (camelCase JSON). */
export type EventsConsentSource =
  | 'migration'
  | 'registration'
  | 'desk'
  | 'profile'
  | 'unsubscribe';

// Update profile request type
export interface UpdateProfileRequest {
  firstname?: string;
  lastname?: string;
  email?: string;
  login?: string;
  addrStreet?: string;
  addrZipCode?: number;
  addrCity?: string;
  phone?: string;
  birthdate?: string;
  currentPassword?: string;
  newPassword?: string;
  language?: string;
  /** Events programme opt-in. The server sets the source from this route. */
  eventsConsent?: boolean;
}

/** First-login / forced password change via POST /auth/change-password */
export interface ChangePasswordRequest {
  newPassword: string;
}

/** POST /auth/request-password-reset (public, no JWT).
 * The server builds the email link from PUBLIC_BASE_URL; the client does not send a URL.
 */
export interface RequestPasswordResetRequest {
  identifier: string;
}

/** POST /auth/reset-password (public, no JWT) */
export interface ResetPasswordFromTokenRequest {
  token: string;
  newPassword: string;
}

/** `UserStatus` JSON (`active` | `blocked` | `deleted`). */
export type UserAccountStatus = 'active' | 'blocked' | 'deleted';

/**
 * Patron summary for staff (`UserShortStaff`).
 * `login` and `barcode` are on this type only. `publicType` is the numeric public-type id.
 */
export interface UserShortStaff {
  id: string;
  /** Sign-in name. Patron pickers show this beside the display name. */
  login?: string | null;
  /** Library card number. Shown when `login` is empty. */
  barcode?: string | null;
  /**
   * Library card on the minimized `GET /loans/:id/user` body.
   * Preferred over `barcode`. Null when the reader has no card.
   */
  cardNumber?: string | null;
  /** Uppercase initials from the minimized loan-user body. No dots. */
  initials?: string | null;
  firstname?: string | null;
  lastname?: string | null;
  accountType?: string | null;
  publicType?: number | null;
  nbLoans?: number | null;
  nbLateLoans?: number | null;
  status?: UserAccountStatus | null;
  createdAt?: string | null;
  expiryAt?: string | null;
}

/** Patron summary for readers (`UserShortPublic`). No `login` or `barcode`. */
export interface UserShortPublic {
  id: string;
  firstname?: string | null;
  lastname?: string | null;
  accountType?: string | null;
  publicType?: number | null;
  nbLoans?: number | null;
  nbLateLoans?: number | null;
  status?: UserAccountStatus | null;
  createdAt?: string | null;
  expiryAt?: string | null;
}

/** Staff list row. Same JSON as {@link UserShortStaff}. */
export type UserShort = UserShortStaff;

/**
 * `loanStatus.blockReasons` on `GET /users/by-card/:cardNumber`.
 * `overdue_loans` does not set `blocked` by itself.
 */
export type PatronBlockReason =
  | 'account_blocked'
  | 'card_lost'
  | 'subscription_expired'
  | 'unpaid_fines'
  | 'max_loans'
  | 'overdue_loans';

/** Circulation snapshot returned with a card scan. */
export interface PatronLoanStatus {
  subscriptionValid: boolean;
  /** Library-local `YYYY-MM-DD`, or null when the patron has no expiry. */
  subscriptionExpiresAt: string | null;
  /** True when checkout without force would refuse this patron. */
  blocked: boolean;
  blockReasons: PatronBlockReason[];
  activeLoans: number;
  overdueLoans: number;
  /** Audience total cap. Null only when no total cap is configured. */
  maxLoans: number | null;
  unpaidFinesCents: number;
  /**
   * Holds ready for pickup. Every site when the agent has no site
   * (users and the JWT store none).
   */
  readyHolds: number;
}

/** Loans staff without users read. `GET /users/by-card/:cardNumber`. */
export interface DeskCardLookup {
  id: string;
  cardNumber: string | null;
  initials: string;
  loanStatus: PatronLoanStatus;
}

/** Users read. Full user plus the same `loanStatus`. */
export type StaffCardLookup = User & { loanStatus: PatronLoanStatus };

export type PatronCardLookup = DeskCardLookup | StaffCardLookup;

export interface LoginRequest {
  username: string;
  password: string;
  deviceId?: string;
}

/**
 * Session user from `GET /auth/me` and the login `user` object.
 * `publicType` is the audience name (`child`, `adult`), not the id.
 * `username` is the same value as `login`.
 */
export interface AuthMeUser extends User {
  login: string;
  username: string;
  accountType: string;
  publicType: string | null;
  guardianId: string | null;
}

export interface LoginResponse {
  token?: string;
  tokenType: string;
  expiresIn: number;
  requires2fa: boolean;
  /** When true, the user must change password before using the app. */
  mustChangePassword?: boolean;
  twoFactorMethod?: string | null;
  deviceId?: string | null;
  user: AuthMeUser;
}

// 2FA Types
export type TwoFactorMethod = 'totp' | 'email';

export interface Setup2FARequest {
  method: TwoFactorMethod;
}

export interface Setup2FAResponse {
  provisioningUri?: string;
  recoveryCodes: string[];
}

export interface Verify2FARequest {
  userId: string;
  code: string;
  trustDevice?: boolean;
  deviceId?: string;
}

export interface Verify2FAResponse {
  token: string;
  tokenType: string;
  expiresIn: number;
  deviceId?: string | null;
  mustChangePassword?: boolean;
}

export interface VerifyRecoveryRequest {
  userId: string;
  code: string;
}

// Media type enum matching server definition (camelCase strings)
export type MediaType =
  | 'all'
  | 'unknown'
  | 'printedText'
  | 'multimedia'
  | 'comics'
  | 'periodic'
  | 'video'
  | 'videoTape'
  | 'videoDvd'
  | 'audio'
  | 'audioMusic'
  | 'audioMusicTape'
  | 'audioMusicCd'
  | 'audioNonMusic'
  | 'audioNonMusicTape'
  | 'audioNonMusicCd'
  | 'cdRom'
  | 'images';

export interface MediaTypeOption {
  value: MediaType | '';
  label: string;
}

// ──────────────────────────────────────────────────────────────────
// Domain: Biblio (notice bibliographique) and Item (exemplaire physique)
//
// Server rename:
//   old Item      → Biblio   (bibliographic record: title, ISBN, authors…)
//   old Specimen  → Item     (physical copy: barcode, location…)
// ──────────────────────────────────────────────────────────────────

export interface Author {
  id: string;
  key?: string | null;
  lastname?: string | null;
  firstname?: string | null;
  bio?: string | null;
  /** Staff author notes. Omitted on {@link AuthorPublic}. */
  notes?: string | null;
  function?: string | null;
}

/** Author embedded on a public catalog record. No staff `notes`. */
export interface AuthorPublic {
  id: string;
  key?: string | null;
  lastname?: string | null;
  firstname?: string | null;
  bio?: string | null;
  function?: string | null;
}

export interface Edition {
  id: string | null;
  publisherName?: string | null;
  placeOfPublication?: string | null;
  date?: string | null;
}

export interface Serie {
  id: string | null;
  key?: string | null;
  name?: string | null;
  issn?: string | null;
  volumeNumber?: number | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface CreateSerie {
  name: string;
  key?: string;
  issn?: string | null;
}

export interface UpdateSerie {
  name?: string;
  issn?: string | null;
}

export interface Collection {
  id: string | null;
  key?: string | null;
  name?: string | null;
  secondaryTitle?: string | null;
  tertiaryTitle?: string | null;
  issn?: string | null;
  volumeNumber?: number | null;
  createdAt?: string | null;
  updatedAt?: string | null;
}

export interface CreateCollection {
  name: string;
  key?: string;
  secondaryTitle?: string | null;
  tertiaryTitle?: string | null;
  issn?: string | null;
}

export interface UpdateCollection {
  name?: string;
  secondaryTitle?: string | null;
  tertiaryTitle?: string | null;
  issn?: string | null;
}

/** Item weeding lifecycle. JSON camelCase. Orthogonal to circulation exceptions and archivedAt. */
export type ItemWeedingStatus = 'onShelf' | 'candidate' | 'withdrawn';

/**
 * Staff copy availability. The server decides this; do not infer it from
 * `borrowed`, `borrowable`, or `circulationStatus`.
 *
 * Precedence: onLoan > inTransit > onHold > toSearch > unavailable > available.
 * `onHold` covers holds in `to_pick` and `ready`. `unavailable` covers incomplete
 * receipt, lost, damaged, claimed returned, and withdrawn.
 */
export type ItemAvailability =
  | 'available'
  | 'onHold'
  | 'inTransit'
  | 'onLoan'
  | 'toSearch'
  | 'unavailable';

/**
 * Availability on `ItemPublicView` / `ItemShortPublic` (`ItemPublicAvailability`).
 * Lost, declared missing, weeding-in-progress, and withdrawn copies are omitted
 * before this value is chosen. `onSiteOnly` is a copy excluded from loan.
 * No reason and no `holdFor`.
 */
export type ItemPublicAvailability = 'available' | 'onLoan' | 'onSiteOnly' | 'unavailable';

/**
 * Biblio weeding mark. Same JSON enum as {@link ItemWeedingStatus}.
 * Derived value is `withdrawn` only when every remaining copy is withdrawn.
 */
export type BiblioWeedingStatus = ItemWeedingStatus;

/** POST /items/{id}/weeding — candidate / onShelf only. `withdrawn` is rejected. */
export interface SetItemWeeding {
  status: ItemWeedingStatus;
  /** Optional short reason (max 200 characters). */
  reason?: string;
}

/** Closed weeding reason. JSON camelCase. */
export type WithdrawalReason =
  | 'worn'
  | 'obsolete'
  | 'newEdition'
  | 'surplus'
  | 'notBorrowed'
  | 'outOfPolicy'
  | 'lostNotReplaced'
  | 'other';

/** Where a weeded copy goes. Null only on legacy rows. */
export type WithdrawalDestination = 'pulp' | 'donation' | 'sale' | 'deposit';

/** Create a withdrawal or complete a legacy row. */
export interface WithdrawalWrite {
  reason: WithdrawalReason;
  reasonDetail?: string | null;
  destination: WithdrawalDestination;
}

export interface BatchWithdrawalRequest {
  itemIds: string[];
  reason: WithdrawalReason;
  reasonDetail?: string | null;
  destination: WithdrawalDestination;
}

/** One withdrawal event. `disposedAt` is null until the copy is marked disposed. */
export interface ItemWithdrawal {
  id: string;
  itemId: string;
  withdrawnAt: string;
  reason: WithdrawalReason;
  reasonDetail: string | null;
  destination: WithdrawalDestination | null;
  agentId: string | null;
  agentName?: string | null;
  cancelledAt: string | null;
  cancelledBy: string | null;
  legacy: boolean;
  /** Set by the dispose batch. Migrated rows carry the withdrawal date. */
  disposedAt?: string | null;
  disposedBy?: string | null;
  /** Display name when the server sends one (same role as `agentName`). */
  disposedByName?: string | null;
  barcode: string | null;
  callNumber: string | null;
  biblioId: string | null;
  biblioTitle: string | null;
  siteId: string | null;
  siteName: string | null;
}

export interface BatchWithdrawalResponse {
  withdrawals: ItemWithdrawal[];
}

export interface WithdrawalReasonTotal {
  reason: WithdrawalReason;
  count: number;
}

export interface WithdrawalDestinationTotal {
  destination: WithdrawalDestination | null;
  count: number;
}

export interface WithdrawalList {
  items: ItemWithdrawal[];
  total: number;
  page: number;
  perPage: number;
  totalsByReason: WithdrawalReasonTotal[];
  totalsByDestination: WithdrawalDestinationTotal[];
}

/** One copy in `candidate` weeding status (`GET /weeding-candidates`). */
export interface WeedingCandidate {
  itemId: string;
  biblioId: string | null;
  barcode: string | null;
  callNumber: string | null;
  biblioTitle: string | null;
  siteId: string | null;
  siteName: string | null;
  weedingReason: string | null;
  markedAt: string;
}

export interface WeedingCandidateList {
  items: WeedingCandidate[];
  total: number;
  page: number;
  perPage: number;
}

export interface WeedingCandidateListParams {
  siteId?: string;
  collectionId?: string;
  callNumber?: string;
  page?: number;
  perPage?: number;
}

/** Query for `GET /item-withdrawals` and the CSV export. */
export interface WithdrawalListParams {
  from?: string;
  to?: string;
  siteId?: string;
  reason?: WithdrawalReason;
  /** `unset` selects a NULL destination. */
  destination?: WithdrawalDestination | 'unset';
  collectionId?: string;
  callNumber?: string;
  page?: number;
  perPage?: number;
  includeCancelled?: boolean;
  /** Omitted means both disposed and not yet disposed. */
  disposed?: boolean;
  /** CSV only: `french` or `english`. */
  lang?: 'french' | 'english';
  /**
   * Not in the published list contract. The item file needs the active row.
   * Ignored by a server that does not know the parameter — do not rely on it
   * for the item file (see `getActiveItemWithdrawal`).
   */
  itemId?: string;
}

export type DepositBatchStatus = 'active' | 'returning' | 'closed';

export type DepositCopyState =
  | 'onShelf'
  | 'onLoan'
  | 'onHold'
  | 'inTransit'
  | 'returned'
  | 'missing';

/** Embedded on staff item JSON. Null means own stock. */
export interface ItemDeposit {
  batchId: string;
  lenderId: string;
  lenderName: string;
  reference: string;
  returnDueOn: string | null;
  batchStatus: DepositBatchStatus;
}

/** Reader and OPAC deposit block. Batch return date is staff-only. */
export interface ItemDepositPublic {
  batchId: string;
  lenderId: string;
  lenderName: string;
  reference: string;
  batchStatus: DepositBatchStatus;
}

export interface Lender {
  id: string;
  name: string;
  contact: string | null;
  notes: string | null;
  /** Integer cents. Null means no lender rate. Never zero. */
  replacementRateCents: number | null;
  lossFlaggedAt: string | null;
  archivedAt: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface LenderWrite {
  name?: string;
  contact?: string | null;
  notes?: string | null;
  /**
   * Integer cents. Null on create means no rate.
   * Omit on update to keep the stored rate. Zero is rejected.
   */
  replacementRateCents?: number | null;
}

export interface DepositBatch {
  id: string;
  lenderId: string;
  reference: string;
  siteId: string;
  arrivedOn: string;
  returnDueOn: string | null;
  status: DepositBatchStatus;
  closedAt: string | null;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface DepositBatchCounts {
  total: number;
  onShelf: number;
  onLoan: number;
  onHold: number;
  inTransit: number;
  returned: number;
}

export interface DepositBatchDetail extends DepositBatch {
  counts: DepositBatchCounts;
}

export interface CreateDepositBatch {
  lenderId: string;
  reference: string;
  siteId: string;
  arrivedOn: string;
  returnDueOn?: string | null;
  notes?: string | null;
}

/** Editable while the batch is active. `siteId` does not move copies. */
export interface UpdateDepositBatch {
  returnDueOn?: string;
  siteId?: string;
  notes?: string | null;
}

export interface DepositBatchItem {
  itemId: string;
  biblioId: string;
  barcode: string | null;
  title: string | null;
  state: DepositCopyState;
}

/** `POST /deposit-batches/scan-out` — batch resolved from the barcode. */
export interface DepositGlobalScanOut {
  itemId: string;
  barcode: string;
  depositReturnedAt: string;
  alreadyScanned: boolean;
  depositBatchId: string;
  batchReference: string;
  lenderName: string;
}

/**
 * Checkout and renewal body. `dueDateCappedByDeposit` is added by server PR 2.
 * It is absent until that PR lands.
 */
export interface LoanMutationResult {
  id: string;
  expiryAt: string;
  message: string;
  dueDateCappedByDeposit?: boolean;
}

/**
 * Copy embedded in a staff list (`ItemShortStaff`).
 * No loan id. `publicNote` and `internalNote` exist only on {@link ItemStaffView}.
 */
export interface ItemShortStaff {
  id: string;
  barcode: string | null;
  callNumber: string | null;
  borrowable: boolean;
  weedingStatus: ItemWeedingStatus;
  siteName: string | null;
  borrowed: boolean;
  /**
   * `items.circulation_status`. Null or 0 is available; 1 lost, 2 damaged,
   * 3 claimed returned, 4 to search, 5 declared missing.
   */
  circulationStatus?: number | null;
  availability: ItemAvailability;
  canHoldItem: boolean;
  /**
   * Patron this copy is held for. Present on an `onHold` copy when the caller
   * has users or loans read. Full {@link UserShortStaff} with users read;
   * `{ id, cardNumber, initials }` with loans read only. Omitted otherwise.
   */
  holdFor?: HoldFor | null;
  /** Null or omitted means own stock. */
  deposit?: ItemDeposit | null;
}

/** @deprecated Use {@link ItemShortStaff}. */
export type ItemShort = ItemShortStaff;

/**
 * Copy embedded in a reader or OPAC list (`ItemShortPublic`).
 * No weeding, circulation status, loan id, `publicNote`, `internalNote`, or hold patron.
 */
export interface ItemShortPublic {
  id: string;
  barcode: string | null;
  callNumber: string | null;
  borrowable: boolean;
  siteName: string | null;
  borrowed: boolean;
  availability: ItemPublicAvailability;
  canHoldItem: boolean;
  /** Null or omitted means own stock. */
  deposit?: ItemDepositPublic | null;
}

/** Staff search hit (`BiblioShortStaff`). */
export interface BiblioShortStaff {
  id: string;
  mediaType: MediaType;
  isbn: string | null;
  title: string | null;
  date: string | null;
  status: number;
  isValid: boolean | null;
  archivedAt: string | null;
  /** `withdrawn` only when every remaining copy is withdrawn. */
  weedingStatus: ItemWeedingStatus;
  author: Author | null;
  items: ItemShortStaff[];
}

/** @deprecated Use {@link BiblioShortStaff}. */
export type BiblioShort = BiblioShortStaff;

/** OPAC/reader search hit (`BiblioShortPublic`). No weeding mark. */
export interface BiblioShortPublic {
  id: string;
  mediaType: MediaType;
  isbn: string | null;
  title: string | null;
  date: string | null;
  status: number;
  isValid: boolean | null;
  archivedAt: string | null;
  author: AuthorPublic | null;
  items: ItemShortPublic[];
}

/** Minimized borrower a future item payload may embed. No name. */
export interface ItemCopyBorrower {
  id?: string | null;
  cardNumber?: string | null;
  initials?: string | null;
}

/** Full physical copy (exemplaire) */
export interface Item {
  id: string;
  biblioId?: string | null;
  siteId?: string | null;
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  /**
   * Item circulation state. Numbers: 0 available, 1 lost, 2 damaged, 3 claimed-returned, 4 to search.
   * Present on {@link ItemStaffView} only. List copies and public views omit it.
   */
  circulationStatus?: number | string | null;
  /**
   * Patron-facing copy note. Staff and public detail payloads include it.
   * List and search rows omit it.
   */
  publicNote?: string | null;
  /**
   * Staff-only copy note. Present on staff detail payloads.
   * Patrons, the OPAC, and list rows omit it.
   */
  internalNote?: string | null;
  price?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  archivedAt?: string | null;
  weedingStatus?: ItemWeedingStatus | null;
  weedingReason?: string | null;
  siteName?: string | null;
  borrowed?: boolean;
  /** Active loan id when borrowed (staff circulation lookup). Never shown as a borrower name. */
  loanId?: string | null;
  /**
   * Future server field. Card and initials only; absent on current item payloads.
   * The item screen must not load the loan to fill this in.
   */
  borrower?: ItemCopyBorrower | null;
  /** Future server field. Expected return, when the copy is on loan. */
  dueDate?: string | null;
  /** Server-computed. May be `toSearch` on staff responses. */
  availability: ItemAvailability;
  /** True only for `available` and `onLoan`, and never when the copy is withdrawn. */
  canHoldItem: boolean;
  /** Staff only, and only when this copy is on hold. See {@link HoldFor}. */
  holdFor?: HoldFor | null;
  /** Null or omitted means own stock. */
  deposit?: ItemDeposit | null;
}

/**
 * Staff notice copy (`ItemStaffView`).
 * Includes weeding, `publicNote`, `internalNote`, circulation status, the active loan id, `dueDate`,
 * `onHold`, and the hold patron when `availability` is `onHold`.
 * `borrower` is present only on `GET /biblios/{id}` when the caller has loans
 * read or write and the copy is on loan. `priceDeferred` is sent only when true.
 */
export interface ItemStaffView {
  id: string | null;
  biblioId: string | null;
  siteId: string | null;
  barcode: string | null;
  /** Lender barcode on deposit copies (staff). Omitted when null. */
  lenderBarcode?: string | null;
  callNumber: string | null;
  volumeDesignation: string | null;
  place: number | null;
  borrowable: boolean;
  circulationStatus: number | null;
  weedingStatus: ItemWeedingStatus;
  weedingReason: string | null;
  /** Copy note shown to patrons and the OPAC. Null when empty. */
  publicNote: string | null;
  /** Staff-only copy note. Former `notes` values live here. */
  internalNote: string | null;
  price: string | null;
  priceDeferred?: boolean;
  createdAt: string | null;
  updatedAt: string | null;
  archivedAt: string | null;
  siteName: string | null;
  borrowed: boolean;
  loanId?: string | null;
  /** Active loan due instant. Null when the copy is not on loan. */
  dueDate: string | null;
  availability: ItemAvailability;
  canHoldItem: boolean;
  /**
   * True only when this copy is `to_pick` or `ready`, or in transit for a hold.
   * An unassigned `pending` title hold leaves this false.
   */
  onHold: boolean;
  /**
   * Set when this copy is on hold and the caller has users or loans read.
   * Users read: {@link UserShortStaff}. Loans read only: {@link LoanBorrowerMinimized}.
   * Readers, the OPAC, and staff with neither right omit it.
   */
  holdFor?: HoldFor | null;
  /**
   * `GET /biblios/{id}` only. Minimized `{ id, cardNumber, initials }` without
   * users read; `UserShortStaff` with users read.
   */
  borrower?: UserShortStaff | LoanBorrowerMinimized | null;
  /** Null means own stock. */
  deposit?: ItemDeposit | null;
}

/**
 * Reader/OPAC notice copy (`ItemPublicView`).
 * Includes `publicNote` only. No `internalNote`, weeding, circulation status, loan id, borrower, hold patron, or `dueDate`.
 * `onHold` and `expectedReturnDate` (date only, on loan, omitted when overdue) are included.
 * `availability` is never `toSearch`.
 */
export interface ItemPublicView {
  id: string | null;
  biblioId: string | null;
  siteId: string | null;
  barcode: string | null;
  callNumber: string | null;
  volumeDesignation: string | null;
  place: number | null;
  borrowable: boolean;
  /** Copy note safe for patrons and the OPAC. Null when the copy has none. */
  publicNote: string | null;
  price: string | null;
  createdAt: string | null;
  updatedAt: string | null;
  archivedAt: string | null;
  siteName: string | null;
  borrowed: boolean;
  /**
   * Library-local calendar date (`YYYY-MM-DD`, no time) of the open loan.
   * Present only when `availability` is `onLoan` and the loan is not overdue.
   * Omitted otherwise, including when overdue. No borrower and no `dueDate`.
   */
  expectedReturnDate?: string;
  availability: ItemPublicAvailability;
  canHoldItem: boolean;
  /**
   * True only when this copy is `to_pick` or `ready`, or in transit for a hold.
   * An unassigned `pending` title hold leaves this false.
   */
  onHold: boolean;
  /** Null or omitted means own stock. Batch return date is staff-only. */
  deposit?: ItemDepositPublic | null;
}

/** `GET /biblios/{id}`, `GET /items/{id}`, and `GET /items/barcode/{barcode}`. */
export type CatalogNotice = BiblioStaffView | BiblioPublicView;

/** Full bibliographic record */
export interface Biblio {
  id?: string | null;
  marcFormat?: string | null;
  mediaType?: MediaType | string | null;
  isbn?: string | null;
  barcode?: string | null;
  callNumber?: string | null;
  price?: string | null;
  title?: string | null;
  /** Genre enum camelCase (e.g. `litteratureFiction`). */
  genre?: string | null;
  subject?: string | null;
  dewey?: string | null;
  audienceType?: string | null;
  lang?: string | null;
  langOrig?: string | null;
  publicationDate?: string | null;
  pageExtent?: string | null;
  format?: string | null;
  tableOfContents?: string | null;
  accompanyingMaterial?: string | null;
  abstract?: string | null;
  notes?: string | null;
  keywords?: string | string[] | null;
  state?: string | null;
  isValid?: number | null;
  seriesIds?: string[];
  seriesVolumeNumbers?: (number | null)[];
  collectionIds?: string[];
  collectionVolumeNumbers?: (number | null)[];
  editionId?: string | null;
  collectionId?: string | null;
  collectionSequenceNumber?: number | null;
  collectionVolumeNumber?: number | null;
  status?: number;
  createdAt?: string | null;
  updatedAt?: string | null;
  archivedAt?: string | null;
  /** Derived: withdrawn only when every remaining copy is withdrawn. */
  weedingStatus?: BiblioWeedingStatus | null;
  authors?: Author[];
  series?: Serie[];
  collections?: Collection[];
  /** Legacy single-collection (kept for backward compat with Z3950 / old data) */
  collection?: Collection | null;
  edition?: Edition | null;
  /** Physical copies of this bibliographic record */
  items?: Item[];
  marcRecord?: unknown;
}

/** Staff bibliographic notice (`BiblioStaffView`). Includes notes and the weeding mark. */
export interface BiblioStaffView {
  id: string | null;
  mediaType: MediaType;
  isbn: string | null;
  title: string | null;
  subject: string | null;
  dewey: string | null;
  /** Staff-only genre for cataloguing / call-number suggestions. */
  genre?: string | null;
  audienceType: string | null;
  lang: string | null;
  langOrig: string | null;
  publicationDate: string | null;
  pageExtent: string | null;
  format: string | null;
  tableOfContents: string | null;
  accompanyingMaterial: string | null;
  /** JSON key `abstract` (`abstract_` on the server). */
  abstract: string | null;
  notes: string | null;
  keywords: string[] | null;
  isValid: boolean | null;
  seriesIds: string[];
  seriesVolumeNumbers: (number | null)[];
  editionId: string | null;
  collectionIds: string[];
  collectionVolumeNumbers: (number | null)[];
  createdAt: string | null;
  updatedAt: string | null;
  archivedAt: string | null;
  weedingStatus: ItemWeedingStatus;
  /** `pending` holds still waiting for a copy. No patron identity. */
  holdsCount: number;
  authors: Author[];
  series: Serie[];
  collections: Collection[];
  edition: Edition | null;
  items: ItemStaffView[];
}

/**
 * Reader/OPAC bibliographic notice (`BiblioPublicView`).
 * No bibliographic notes, weeding mark, or staff copy fields.
 */
export interface BiblioPublicView {
  id: string | null;
  mediaType: MediaType;
  isbn: string | null;
  title: string | null;
  subject: string | null;
  dewey: string | null;
  audienceType: string | null;
  lang: string | null;
  langOrig: string | null;
  publicationDate: string | null;
  pageExtent: string | null;
  format: string | null;
  tableOfContents: string | null;
  accompanyingMaterial: string | null;
  /** JSON key `abstract` (`abstract_` on the server). */
  abstract: string | null;
  keywords: string[] | null;
  isValid: boolean | null;
  seriesIds: string[];
  seriesVolumeNumbers: (number | null)[];
  editionId: string | null;
  collectionIds: string[];
  collectionVolumeNumbers: (number | null)[];
  createdAt: string | null;
  updatedAt: string | null;
  archivedAt: string | null;
  /** `pending` holds still waiting for a copy. No patron identity. */
  holdsCount: number;
  authors: AuthorPublic[];
  series: Serie[];
  collections: Collection[];
  edition: Edition | null;
  items: ItemPublicView[];
}

/** Physical item data when creating a Biblio in one request (POST /biblios with items) */
export interface CreateBiblioItemInput {
  barcode?: string | null;
  callNumber?: string | null;
  siteId: string;
}

/** Payload for POST /biblios/{id}/items */
export interface CreateItem {
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  /** Optional. Blank is stored as null. Max 4000 characters. */
  publicNote?: string | null;
  /** Optional staff note. Blank is stored as null. Max 4000 characters. */
  internalNote?: string | null;
  price?: string | null;
  /** Write-only: treat a missing price as an explicit deferral when enabling circulation. */
  priceDeferred?: boolean | null;
  siteId?: string | null;
  siteName?: string | null;
}

/** Payload for PUT /items/{id} */
export interface UpdateItem {
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  /**
   * Omit or `null` to keep the stored note. `""` clears it.
   * Longer than 4000 characters is HTTP 400 `validation_error`.
   */
  publicNote?: string | null;
  /** Same omit, null, and blank rules as `publicNote`. Staff only. */
  internalNote?: string | null;
  price?: string | null;
  /** Write-only: treat a missing price as an explicit deferral when enabling circulation. */
  priceDeferred?: boolean | null;
  siteId?: string | null;
  siteName?: string | null;
}

// Circulation exceptions (lost / damaged / claimed-returned) — staff desk API
export type DamageDisposition = 'return' | 'keep';
export type ClaimsResolveOutcome = 'found' | 'notFound';
export type CirculationStatusName = 'available' | 'lost' | 'damaged' | 'claimedReturned';
export type CirculationExceptionOutcome =
  | 'lost'
  | 'damaged'
  | 'claimedReturned'
  | 'claimsResolvedFound'
  | 'claimsResolvedNotFound';

export interface MarkLostRequest {
  bill?: boolean;
  amount?: string;
  useItemPrice?: boolean;
  notes?: string;
}

export interface MarkDamagedRequest {
  disposition: DamageDisposition;
  bill?: boolean;
  amount?: string;
  notes?: string;
}

export interface MarkClaimedReturnedRequest {
  notes?: string;
}

export interface ResolveClaimsReturnedRequest {
  outcome: ClaimsResolveOutcome;
  inventoryChecked: boolean;
  notes?: string;
}

export interface CirculationExceptionResponse {
  outcome: CirculationExceptionOutcome;
  itemStatus: CirculationStatusName;
  borrowable: boolean;
  loanClosed: boolean;
  loanId: string;
  itemId: string;
  userId: string;
  charge?: Fine | null;
}

export interface ClaimsReturnedQueueItem {
  loanId: string;
  itemId: string;
  userId: string;
  barcode?: string | null;
  title?: string | null;
  loanStart: string;
  expiryAt?: string | null;
  itemUpdatedAt?: string | null;
  notes?: string | null;
}

/** Why a closed loan ended. Wire values from `LoanClosureReason`. */
export type LoanClosureReason = 'returned' | 'lost';

/**
 * Why `GET /me/loans` refuses a renewal (`MyLoanRenewalRefusal`).
 * Snake_case. Absent (`null`) when `canRenew` is true.
 * Account reasons (`card_lost`, `account_blocked`, `subscription_expired`,
 * `fines_over_threshold`) are checked first and apply to every loan.
 * Staff `force` skips `account_blocked`, `subscription_expired`, and
 * `fines_over_threshold`. It does not skip `card_lost`.
 */
export type MyLoanRenewalRefusal =
  | 'card_lost'
  | 'account_blocked'
  | 'subscription_expired'
  | 'fines_over_threshold'
  | 'withdrawn'
  | 'claimed_returned'
  | 'hold_waiting'
  | 'max_renewals'
  | 'renewal_too_early';

/**
 * `GET /me/loans` — the caller's own active loans (`MyLoanView`).
 * Does not embed a catalog biblio or copy.
 */
export interface MyLoan {
  id: string;
  barcode: string | null;
  title: string | null;
  mediaType: MediaType;
  startDate: string;
  dueDate: string;
  renewalCount: number;
  canRenew: boolean;
  renewalRefusalCode: MyLoanRenewalRefusal | null;
  /** Library-local `YYYY-MM-DD` when `renewalRefusalCode` is `renewal_too_early`. */
  renewalOpensAt: string | null;
  isOverdue: boolean;
  /** Set on `GET /me/loans?archived=true`. Omitted on open loans. */
  closureReason?: LoanClosureReason | null;
  /** When a loan closed as `lost` had its copy found. Null until then. */
  foundAt?: string | null;
}

/** Loan list row for staff (`LoanDetailsStaff`). Embedded copies are {@link BiblioShortStaff}. */
export interface LoanDetailsStaff {
  id: string;
  itemId: string;
  startDate: string;
  expiryAt: string;
  renewalDate: string | null;
  nbRenews: number;
  returnedAt: string | null;
  /** Set on archived loans. Omitted on open loans. */
  closureReason?: LoanClosureReason | null;
  /** Timestamp when a lost copy was found, or null until then. */
  foundAt?: string | null;
  biblio: BiblioShortStaff;
  user: UserShortStaff | null;
  itemIdentification: string | null;
  isOverdue: boolean;
}

/** Borrower on a loan row when the caller cannot read users. */
export interface LoanBorrowerMinimized {
  id: string;
  cardNumber: string | null;
  initials: string;
}

/**
 * Patron a copy is held for, or the reader on a staff hold row.
 * Users read: {@link UserShortStaff}. Loans read only: {@link LoanBorrowerMinimized}.
 */
export type HoldFor = UserShortStaff | LoanBorrowerMinimized;

/** Loan list row without users read (`LoanDetailsOwn`). Embedded copies are {@link BiblioShortPublic}. */
export interface LoanDetailsPublic {
  id: string;
  itemId: string;
  startDate: string;
  expiryAt: string;
  renewalDate: string | null;
  nbRenews: number;
  returnedAt: string | null;
  /** Set on archived loans. Omitted on open loans. */
  closureReason?: LoanClosureReason | null;
  /** Timestamp when a lost copy was found, or null until then. */
  foundAt?: string | null;
  biblio: BiblioShortPublic;
  user: LoanBorrowerMinimized | null;
  itemIdentification: string | null;
  isOverdue: boolean;
}

/** `GET /users/{id}/loans` — staff or the reader's own loans. */
export type LoanListItem = LoanDetailsStaff | LoanDetailsPublic;

/** Staff loan row. Same JSON as {@link LoanDetailsStaff}. */
export type Loan = LoanDetailsStaff;

// Stats types
export interface Stats {
  biblios: {
    total: number;
    byMediaType: StatEntry[];
    byPublicType: StatEntry[];
    acquisitions?: number;
    acquisitionsByMediaType?: StatEntry[];
    withdrawals?: number;
    withdrawalsByMediaType?: StatEntry[];
  };
  users: {
    total: number;
    active: number;
    byAccountType: StatEntry[];
  };
  loans: {
    active: number;
    overdue: number;
    returnedToday: number;
    byMediaType: StatEntry[];
  };
}

export interface StatEntry {
  label: string;
  value: number;
  acquisitions?: number;
  eliminations?: number;
}

/** Aggregate user stats from `GET /stats/users`. No named borrowers. */
export interface UserAggregateStats {
  newUsersTotal: number;
  activeBorrowersTotal: number;
  usersTotal: number;
  newUsersByPublicType?: StatEntry[];
  activeBorrowersByPublicType?: StatEntry[];
  usersByPublicType?: StatEntry[];
  usersByAgeBand?: StatEntry[];
  usersByAccountType?: StatEntry[];
  usersBySex?: StatEntry[];
  newUsersBySex?: StatEntry[];
  /** Group accounts (`account_type = group`). */
  groupsTotal?: number;
}

// Time-based stats for charts
export interface LoanTimeStats {
  date: string;
  loans: number;
  returns: number;
}

// Catalog stats from /stats/catalog
export interface CatalogStatsBreakdown {
  label?: string;
  siteId?: string;
  siteName?: string;
  activeItems: number;
  enteredItems: number;
  archivedItems: number;
  loans: number;
  byMediaType?: CatalogStatsBreakdown[] | null;
  byPublicType?: CatalogStatsBreakdown[] | null;
}

export interface CatalogStats {
  totals: {
    activeItems: number;
    enteredItems: number;
    archivedItems: number;
    loans: number;
  };
  bySource?: CatalogStatsBreakdown[] | null;
  byMediaType?: CatalogStatsBreakdown[] | null;
  byPublicType?: CatalogStatsBreakdown[] | null;
}

// Advanced stats types
export type StatsInterval = 'day' | 'week' | 'month' | 'year';

export interface AdvancedStatsParams {
  startDate: string;
  endDate: string;
  interval?: StatsInterval;
  mediaType?: MediaType;
  userId?: string;
  publicType?: string;
}

export interface LoanStatsTimeSeries {
  period: string;
  loans: number;
  returns: number;
}

export interface LoanStatsResponse {
  totalLoans: number;
  totalReturns: number;
  timeSeries: LoanStatsTimeSeries[];
  byMediaType: StatEntry[];
}

// Flexible stats builder (`GET /stats/schema`, `POST /stats/query`, `/stats/saved`)
export type StatsFilterOperator =
  | 'eq'
  | 'neq'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'in'
  | 'notIn'
  | 'isNull'
  | 'isNotNull';

export type StatsAggregateFunction = 'count' | 'countDistinct' | 'sum' | 'avg' | 'min' | 'max';

export type StatsTimeGranularity = 'day' | 'week' | 'month' | 'quarter' | 'year';

export interface StatsSelectField {
  field: string;
  alias?: string | null;
}

export interface StatsGroupByField {
  field: string;
  alias?: string | null;
}

export interface StatsFilterClause {
  field: string;
  op: StatsFilterOperator;
  value: unknown;
}

export interface StatsHavingFilter {
  field: string;
  op: StatsFilterOperator;
  value: unknown;
}

export interface StatsAggregation {
  fn: StatsAggregateFunction;
  field: string;
  alias: string;
}

export interface StatsTimeBucket {
  field: string;
  granularity: StatsTimeGranularity;
  alias?: string | null;
}

export interface StatsOrderBy {
  field: string;
  dir?: 'asc' | 'desc' | null;
}

export interface StatsBuilderBody {
  entity: string;
  joins: string[];
  select: StatsSelectField[];
  filters: StatsFilterClause[];
  /**
   * OR-of-AND groups combined with top-level `filters` as:
   * `(AND filters) AND ((AND g0) OR (AND g1) OR …)`.
   */
  filterGroups?: StatsFilterClause[][];
  /**
   * Additional root tables to combine with `entity` via UNION ALL (e.g. `loans` + `loans_archives`).
   * Discovery lists allowed branches on `entities.<name>.unionWith`.
   */
  unionWith?: string[];
  aggregations: StatsAggregation[];
  groupBy: StatsGroupByField[];
  having: StatsHavingFilter[];
  timeBucket?: StatsTimeBucket | null;
  orderBy: StatsOrderBy[];
  limit?: number | null;
  offset?: number | null;
}

export interface StatsColumnMeta {
  name: string;
  label: string;
  dataType: string;
}

export interface StatsTableResponse {
  columns: StatsColumnMeta[];
  rows: Record<string, unknown>[];
  totalRows: number;
  limit: number;
  offset: number;
}

export interface StatsSchemaRelation {
  join: [string, string];
  label: string;
}

/** Field metadata from `GET /stats/schema` (computed = SQL expression, not a physical column). */
export interface StatsSchemaField {
  type: string;
  label: string;
  computed?: boolean;
}

export interface StatsSchemaEntity {
  label: string;
  fields: Record<string, StatsSchemaField>;
  relations: Record<string, StatsSchemaRelation>;
  /** Additional roots that can be UNION ALL’d with this entity (e.g. `["loans_archives"]` for `loans`). */
  unionWith?: string[];
}

export interface StatsSchema {
  entities: Record<string, StatsSchemaEntity>;
  aggregationFunctions: string[];
  operators: string[];
  timeGranularities: string[];
  /** Human-readable explanation of `filterGroups` OR-of-AND semantics (from server). */
  filterGroupsSemantics?: string;
  /** Explains `unionWith` / multi-root UNION ALL (from server). */
  unionWithSemantics?: string;
}

export interface SavedStatsQuery {
  id: number;
  name: string;
  description?: string | null;
  query: StatsBuilderBody;
  userId: number;
  isShared: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SavedStatsQueryWrite {
  name: string;
  description?: string | null;
  query: StatsBuilderBody;
  isShared: boolean;
}

/** Normalized client shape; server JSON uses `per_page` and `page_count` (see `normalizePaginatedResponse`). */
export interface PaginatedResponse<T> {
  items: T[];
  total: number;
  page: number;
  perPage: number;
  pageCount: number;
}

export interface ImportReport {
  action: 'created' | 'mergedBibliographic' | 'replacedArchived' | 'replacedConfirmed';
  existingId?: string;
  warnings: string[];
  message?: string;
}

export interface ImportResult<T> {
  /** Imported bibliographic record */
  biblio: T;
  importReport: ImportReport;
}

// UNIMARC batch upload / import (POST load-marc, GET marc-batch/:id)
/** Single validation issue from marc-rs `Record::validation_issues` (JSON: camelCase). */
export interface RecordValidationIssue {
  tag: string;
  /** Subfield code when applicable (serialized as a one-character string) */
  subfield?: string | null;
  targetPath: string;
  value: string;
  pattern: string;
}

/** BiblioShort-shaped preview plus optional MARC parse validation diagnostics. */
export type MarcImportPreview = BiblioShort & {
  validationIssues: RecordValidationIssue[];
};

export interface EnqueueResult {
  /** Unique batch identifier in Redis (stringified i64) */
  batchId: string;
  /** One entry per cached notice (same order as upload for load-marc) */
  previews: MarcImportPreview[];
}

export interface MarcBatchImportError {
  key: string;
  error: string;
  /** ID of the existing biblio that caused a duplicate ISBN conflict */
  existingId?: string | null;
  /** Stable failure code. `barcode_already_exists` skips that copy. */
  code?: string | null;
  /** Copy that already uses this barcode. Set with `barcode_already_exists`. */
  existingItemId?: string | null;
}

export interface MarcBatchAttached {
  key: string;
  biblioId: string;
}

export type DepositDuplicateConflictKind = 'activeBarcode' | 'reactivation' | 'wasLost';

export type DepositDuplicateAction = 'overwrite' | 'relabel' | 'ignore';

export interface DepositDuplicateConflict {
  recordKey: string;
  lenderBarcode: string;
  kind: DepositDuplicateConflictKind;
  existingItemId: string;
  existingBarcode?: string | null;
}

export interface DepositDuplicateScanReport {
  batchId: string;
  conflicts: DepositDuplicateConflict[];
}

export interface MarcDepositCopyLine {
  recordKey: string;
  lenderBarcode: string;
  itemId: string;
  barcode?: string;
  localBarcode?: string;
}

export interface MarcBatchImportReport {
  batchId: string;
  /** IDs of successfully imported records. Union of created and attached. */
  imported: string[];
  /** Records whose bibliographic notice was created. Empty outside deposit mode. */
  created?: string[];
  /** Records whose copies were attached to an existing ISBN. */
  attached?: MarcBatchAttached[];
  failed: MarcBatchImportError[];
  copiesCreated?: MarcDepositCopyLine[];
  copiesReactivated?: MarcDepositCopyLine[];
  copiesRelabeled?: MarcDepositCopyLine[];
  copiesOverwritten?: MarcDepositCopyLine[];
  copiesRefused?: Array<{ recordKey: string; lenderBarcode: string; code: string; itemId?: string | null }>;
}

export interface MarcBatchInfo {
  /** Unique batch identifier in Redis (stringified i64) */
  batchId: string;
  recordCount: number;
  /** Redis TTL semantics: -1 no expiry, -2 key missing/expired */
  ttlSeconds: number;
}

export interface DuplicateConfirmationRequired {
  code: 'duplicate_isbn_needs_confirmation';
  existingId: string;
  message: string;
}

// Background tasks (async long-running operations)
export type TaskKind =
  | 'marcBatchImport'
  | 'maintenance'
  | 'inventoryBatchScan'
  | 'inventoryConsolidation';
export type TaskStatus = 'pending' | 'running' | 'completed' | 'failed';

export interface TaskProgress {
  current: number;
  total: number;
  /** Plain text or structured payload from the server (varies by task kind). */
  message?: unknown;
}

export interface InventoryConsolidationProgressMessage {
  sessionId?: string;
  phase?: 'archiving' | 'notifying_readers';
  deleted?: number;
  skipped?: number;
  archivedBiblios?: number;
  recipientCount?: number;
}

export interface TaskStartResponse {
  taskId: string;
}

export interface BackgroundTask {
  id: string;
  kind: TaskKind;
  status: TaskStatus;
  progress?: TaskProgress | null;
  result?:
    | MarcBatchImportReport
    | MaintenanceResponse
    | InventoryScan[]
    | InventoryConsolidationResult
    | null;
  error?: string | null;
  createdAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
  userId: string;
}

// Error response — code is now a string
export interface ApiError {
  code: string;
  error: string;
  message: string;
}

export type ApiErrorCode =
  | 'authentication_failed'
  | 'authorization_failed'
  | 'not_found'
  | 'validation_error'
  | 'bad_request'
  | 'conflict'
  | 'business_rule_violation'
  | 'subscription_expired'
  | 'fines_over_threshold'
  | 'max_loans'
  | 'max_media_loans'
  | 'item_not_loanable'
  | 'item_held_for_other'
  | 'set_aside_item_mismatch'
  | 'set_aside_wrong_title'
  | 'set_aside_not_oldest'
  | 'item_status_action_required'
  | 'duplicate_isbn_needs_confirmation'
  | 'duplicate_barcode_needs_confirmation'
  | 'z3950_error'
  | 'database_error'
  | 'internal_error'
  | 'rate_limited'
  | 'item_to_pick_for_other'
  | 'item_on_loan_to_other_borrower'
  | 'item_already_on_loan_to_borrower'
  | 'deposit_return_due'
  | 'deposit_renewal_no_extension'
  | 'deposit_batch_returning'
  | 'deposit_item_delete_needs_confirmation'
  | 'deposit_item_delete_use_batch_return'
  | 'deposit_item_delete_unpaid_fine'
  | 'item_delete_unpaid_fine'
  | 'deposit_item_not_owned'
  | 'deposit_item_returned_to_lender'
  | 'deposit_item_on_loan'
  | 'deposit_item_was_lost'
  | 'deposit_loss_not_declared'
  | 'deposit_loss_already_notified'
  | 'deposit_site_mismatch'
  | 'deposit_batch_not_active'
  | 'deposit_import_options_conflict'
  | 'lender_has_active_batches'
  | 'barcode_already_exists'
  | 'deposit_batch_has_active_loans'
  | 'deposit_batch_not_returning'
  | 'deposit_replacement_amount_required'
  | 'item_not_deposit_copy';

// Settings — renewal anchoring (serialized wire values: now, at_due_date)
export type LoanSettingsRenewAt = 'now' | 'at_due_date';

/** @deprecated Use LoanSettingsRenewAt */
export type LoanRenewAt = LoanSettingsRenewAt;

export interface LoanSettings {
  /** null = global default row (all media types not overridden) */
  mediaType: MediaType | null;
  /**
   * Default row: max active loans across all media types (after server resolution).
   * Per–media row: cap for that document type only.
   */
  maxLoans: number;
  maxRenewals: number;
  durationDays: number;
  renewAt: LoanSettingsRenewAt;
  /**
   * Days before the due date when a renewal is allowed (0–366).
   * `0` allows renewal only on the due date or once overdue. Default 7.
   */
  renewWindowDays: number;
}

/** Body for PUT /loans/settings (camelCase). */
export interface UpdateLoanSettingsRequest {
  loanSettings: LoanSettings[];
}

export interface Settings {
  loanSettings: LoanSettings[];
  z3950Servers: Z3950Server[];
}

export interface Z3950Server {
  id: string;
  name: string;
  address: string;
  port: number;
  database?: string;
  format?: string;
  login?: string;
  password?: string;
  encoding?: string;
  isActive: boolean;
}

// Public types (audience types for users)
/** Row from GET /genres (DB catalog). */
export interface GenreCatalogEntry {
  /** Enum camelCase (e.g. `litteratureFiction`). */
  code: string;
  shelfCode?: string | null;
  /** Language code → label (`fr` / `en` / `de` / `es`). */
  labels: Record<string, string>;
  synonyms: string[];
  sortOrder: number;
}

export interface PublicType {
  id: string;
  name: string;
  label: string;
  subscriptionDurationDays?: number | null;
  ageMin?: number | null;
  ageMax?: number | null;
  subscriptionPrice?: number | null;
  maxLoans?: number | null;
  loanDurationDays?: number | null;
  /** `inherit` uses the global mode. `none` never blocks on amount. `amount` uses `unpaidThresholdCents`. */
  unpaidThresholdMode: AudienceUnpaidThresholdMode;
  /** Integer cents. Present only when `unpaidThresholdMode` is `amount`. */
  unpaidThresholdCents?: number | null;
  /** Override of global max active holds. null inherits circulation policy. */
  maxActiveHolds?: number | null;
}

/**
 * One row in PUT /public-types/:id/loan-settings body.settings (camelCase).
 * Use mediaType: null for the single audience default row; omit empty strings.
 */
export interface PublicTypeLoanSettingInput {
  mediaType?: MediaType | null;
  duration: number;
  nbMax: number;
  nbRenews: number;
  /** null = inherit global renewAt for this mediaType */
  renewAt?: LoanSettingsRenewAt | null;
}

export interface ReplacePublicTypeLoanSettingsRequest {
  settings: PublicTypeLoanSettingInput[];
}

export interface PublicTypeLoanSettings {
  id: string;
  publicTypeId: string;
  /** null = audience default row (all media types for this profile) */
  mediaType: MediaType | null;
  duration: number;
  /**
   * Default row: max active loans across all media types for this profile (after server resolution).
   * Per–media row: cap for that document type only.
   */
  nbMax: number;
  nbRenews: number;
  /** null = inherit global loan setting for this mediaType (or global default when mediaType is null) */
  renewAt: LoanSettingsRenewAt | null;
}

export interface CreatePublicType {
  name: string;
  label: string;
  subscriptionDurationDays?: number | null;
  ageMin?: number | null;
  ageMax?: number | null;
  subscriptionPrice?: number | null;
  maxLoans?: number | null;
  loanDurationDays?: number | null;
  /** Omitted defaults to `inherit`. `amount` requires `unpaidThresholdCents`. */
  unpaidThresholdMode?: AudienceUnpaidThresholdMode;
  unpaidThresholdCents?: number | null;
  /** Omit or null to inherit the global holds policy. */
  maxActiveHolds?: number | null;
}

export interface UpdatePublicType {
  name?: string;
  label?: string;
  subscriptionDurationDays?: number | null;
  ageMin?: number | null;
  ageMax?: number | null;
  subscriptionPrice?: number | null;
  maxLoans?: number | null;
  loanDurationDays?: number | null;
  /**
   * Omit to leave the stored mode and cents unchanged.
   * `amount` requires `unpaidThresholdCents` in the same body.
   */
  unpaidThresholdMode?: AudienceUnpaidThresholdMode;
  unpaidThresholdCents?: number | null;
  /**
   * Max-active-holds override. JSON `null` clears it (global cap applies).
   * Omit the field to keep the stored value.
   */
  maxActiveHolds?: number | null;
}

// Source type
export interface Site {
  id: string;
  key: string | null;
  name: string | null;
  isArchive?: number | null;
  archivedAt?: string | null;
  default?: boolean;
}

/** Library role with permission flags — GET /account-types (camelCase JSON array). */
export interface AccountTypeDefinition {
  code: string;
  /** Nullable in the API (`account_types.name`). */
  name: string | null;
  itemsRights: string | null;
  usersRights: string | null;
  loansRights: string | null;
  itemsArchiveRights: string | null;
  /** Holds + transits. Server values: `n` / `o` / `r` / `w`. */
  holdsRights: string | null;
  settingsRights: string | null;
  eventsRights: string | null;
  /** Present after the acquisitions domain lands (`n`/`r`/`w`). */
  acquisitionsRights?: string | null;
  /** Weeding (`n`/`r`/`w`). */
  weedingRights?: string | null;
  /** Cancel a withdrawal (`n`/`r`/`w`). */
  weedingCancelRights?: string | null;
  /**
   * Desk override (`n`/`r`/`w`). Write may force a loan past a copy held for
   * another reader and past an expired subscription. Distinct from settings.
   */
  circulationOverrideRights?: string | null;
  /**
   * Built-in types (`guest`, `reader`, `librarian`, `admin`, `group`) are true.
   * Custom types are false. Absent when the server has not sent the field.
   */
  isSystem?: boolean;
}

export type AccountTypeRightLevel = 'n' | 'r' | 'w';

/** Holds, users, and loans rights include `o` (own). */
export type AccountTypeHoldsRightLevel = AccountTypeRightLevel | 'o';

/** PUT /account-types/:code — partial body. Omit a field to leave it unchanged. */
export interface UpdateAccountTypeRequest {
  name?: string;
  itemsRights?: AccountTypeRightLevel | null;
  usersRights?: AccountTypeHoldsRightLevel | null;
  loansRights?: AccountTypeHoldsRightLevel | null;
  itemsArchiveRights?: AccountTypeRightLevel | null;
  holdsRights?: AccountTypeHoldsRightLevel | null;
  settingsRights?: AccountTypeRightLevel | null;
  eventsRights?: AccountTypeRightLevel | null;
  acquisitionsRights?: AccountTypeRightLevel | null;
  weedingRights?: AccountTypeRightLevel | null;
  weedingCancelRights?: AccountTypeRightLevel | null;
  circulationOverrideRights?: AccountTypeRightLevel | null;
}

/** POST /account-types — custom type. Omitted rights are stored as `n`. */
export interface CreateAccountTypeRequest {
  code: string;
  name: string;
  itemsRights?: AccountTypeRightLevel | null;
  usersRights?: AccountTypeHoldsRightLevel | null;
  loansRights?: AccountTypeHoldsRightLevel | null;
  itemsArchiveRights?: AccountTypeRightLevel | null;
  holdsRights?: AccountTypeHoldsRightLevel | null;
  settingsRights?: AccountTypeRightLevel | null;
  eventsRights?: AccountTypeRightLevel | null;
  acquisitionsRights?: AccountTypeRightLevel | null;
  weedingRights?: AccountTypeRightLevel | null;
  weedingCancelRights?: AccountTypeRightLevel | null;
  circulationOverrideRights?: AccountTypeRightLevel | null;
}

// Account types for permissions
export type AccountType = 'Guest' | 'Reader' | 'Librarian' | 'Administrator';

export const isAdmin = (accountType?: string): boolean => {
  const n = accountType?.trim().toLowerCase();
  return n === 'admin' || n === 'administrator';
};

export const isLibrarian = (accountType?: string): boolean => {
  const normalized = accountType?.trim().toLowerCase();
  return normalized === 'admin' || normalized === 'librarian';
};

type RightsUser = Pick<
  User,
  | 'itemsRights'
  | 'usersRights'
  | 'loansRights'
  | 'holdsRights'
  | 'settingsRights'
  | 'eventsRights'
  | 'weedingRights'
  | 'weedingCancelRights'
  | 'itemsArchiveRights'
  | 'acquisitionsRights'
  | 'rights'
> | null | undefined;

/** `own` ranks below `read`. Missing claims are deny. */
export function resolveUsersRights(
  user: Pick<User, 'usersRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): 'n' | 'o' | 'r' | 'w' | null {
  const fromJwt = authToken ? readUsersRightsFromJwt(authToken) : null;
  if (fromJwt) return fromJwt;
  return normalizeUsersRightsLevel(
    (user?.rights && typeof user.rights === 'object' ? user.rights.usersRights : null) ??
      user?.usersRights,
  );
}

function resolveOwnableRight(
  user: RightsUser,
  authToken: string | null | undefined,
  field: 'loansRights' | 'settingsRights' | 'eventsRights',
): 'n' | 'o' | 'r' | 'w' | null {
  const fromJwt = authToken ? readOwnableRightFromJwt(authToken, field) : null;
  if (fromJwt) return fromJwt;
  if (!user) return null;
  const nested = user.rights && typeof user.rights === 'object' ? user.rights : null;
  return normalizeUsersRightsLevel(nested?.[field] ?? user[field]);
}

function atLeastRead(level: 'n' | 'o' | 'r' | 'w' | null): boolean {
  return level === 'r' || level === 'w';
}

/** Catalog write (`itemsRights` write). Patrons share `read` and stay on the public view. */
export const canManageItems = (user: RightsUser, authToken?: string | null): boolean =>
  resolveNamedRight(user, authToken, 'itemsRights', readItemsRightsFromJwt) === 'w';

/** `itemsRights` read or write. Readers have read; it does not select the staff catalog. */
export const canReadItems = (user: RightsUser, authToken?: string | null): boolean => {
  const level = resolveNamedRight(user, authToken, 'itemsRights', readItemsRightsFromJwt);
  return level === 'r' || level === 'w';
};

/** Borrower-record mutations (`usersRights` write). */
export const canManageUsers = (user: RightsUser, authToken?: string | null): boolean =>
  resolveUsersRights(user, authToken) === 'w';

/**
 * Staff may open another borrower's record.
 * JWT `rights.usersRights` wins. `read` and `write` qualify. `own` does not.
 */
export function canReadUsers(
  user: Pick<User, 'usersRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean {
  return atLeastRead(resolveUsersRights(user, authToken));
}

/**
 * Reader updating their own account (`usersRights` own, or no users right).
 * `read` and `write` still use the staff profile form.
 */
export function ownContactOnly(
  user: Pick<User, 'usersRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean {
  const level = resolveUsersRights(user, authToken);
  return level !== 'r' && level !== 'w';
}

/** Desk circulation (`loansRights` read or write). `own` is the patron's loans. */
export const canReadLoans = (user: RightsUser, authToken?: string | null): boolean =>
  atLeastRead(resolveOwnableRight(user, authToken, 'loansRights'));

export const canManageLoans = (user: RightsUser, authToken?: string | null): boolean =>
  resolveOwnableRight(user, authToken, 'loansRights') === 'w';

/** Staff holds, pick list, and transits. `own` does not qualify. */
export function canReadHoldsStaff(user: RightsUser, authToken?: string | null): boolean {
  return atLeastRead(normalizeUsersRightsLevel(resolveHoldsRights(user, authToken)));
}

/** Place a hold for oneself (`own`) or for another reader (`write`). */
export function canCreateHold(user: RightsUser, authToken?: string | null): boolean {
  const level = normalizeUsersRightsLevel(resolveHoldsRights(user, authToken));
  return level === 'o' || level === 'w';
}

/** Place or cancel a hold for another reader. */
export function canWriteHolds(user: RightsUser, authToken?: string | null): boolean {
  return normalizeUsersRightsLevel(resolveHoldsRights(user, authToken)) === 'w';
}

export const canReadSettings = (user: RightsUser, authToken?: string | null): boolean =>
  atLeastRead(resolveOwnableRight(user, authToken, 'settingsRights'));

export const canWriteSettings = (user: RightsUser, authToken?: string | null): boolean =>
  resolveOwnableRight(user, authToken, 'settingsRights') === 'w';

export const canWriteEvents = (user: RightsUser, authToken?: string | null): boolean =>
  resolveOwnableRight(user, authToken, 'eventsRights') === 'w';

/**
 * Staff catalog JSON (#217). Account type is ignored.
 * `itemsRights` read alone stays on the public view.
 */
export function seesInternalCatalog(user: RightsUser, authToken?: string | null): boolean {
  return (
    canManageItems(user, authToken) ||
    canViewWeeding(user, authToken) ||
    canReadLoans(user, authToken) ||
    canReadHoldsStaff(user, authToken) ||
    canReadUsers(user, authToken) ||
    canArchiveItems(user, authToken)
  );
}

/**
 * Staff may send `force` on checkout, batch loan, and renewal.
 * The API requires `loans_rights` write. Read and own do not.
 */
export function canForceLoans(
  user: Pick<User, 'loansRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean {
  const fromJwt = authToken ? readLoansRightsFromJwt(authToken) : null;
  const level =
    fromJwt ??
    normalizeUsersRightsLevel(
      (user?.rights && typeof user.rights === 'object' ? user.rights.loansRights : null) ??
        user?.loansRights,
    );
  return level === 'w';
}

/** Normalized holds level from profile only (`none` | `own` | `read` | `write`). */
export function resolveHoldsRightsFromProfile(
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
): string | null {
  if (!user) return null;
  const nested = user.rights && typeof user.rights === 'object' ? user.rights : null;
  const raw = nested?.holdsRights ?? nested?.borrowsRights ?? user.holdsRights;
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  return s || null;
}

/**
 * Effective holds permission: JWT `rights.holdsRights` first (available right after login),
 * then GET /auth/me fields when present.
 */
export function resolveHoldsRights(
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): string | null {
  const fromJwt = authToken ? readHoldsRightsFromJwt(authToken) : null;
  const fromProfile = resolveHoldsRightsFromProfile(user);
  return fromJwt ?? fromProfile ?? null;
}

/** Personal holds UI (Mes réservations, réserver un exemplaire) for `own`, `read`, or `write`. */
export const canPatronSelfServiceHolds = (
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean => {
  const level = normalizeUsersRightsLevel(resolveHoldsRights(user, authToken));
  return level === 'o' || level === 'r' || level === 'w';
};

/** Library statistics. Same staff view as the internal catalog (#217). */
export const canViewStats = (user: RightsUser, authToken?: string | null): boolean =>
  seesInternalCatalog(user, authToken);

/** Email templates and other settings mutations (`settingsRights` write). */
export const canManageSettings = (user: RightsUser, authToken?: string | null): boolean =>
  canWriteSettings(user, authToken);

/** Normalized acquisitions level: `n` | `r` | `w`. */
export function normalizeAcquisitionsRightsLevel(raw: unknown): AccountTypeRightLevel | null {
  const s = raw != null ? String(raw).trim().toLowerCase() : '';
  if (s === 'n' || s === 'none') return 'n';
  if (s === 'r' || s === 'read') return 'r';
  if (s === 'w' || s === 'write') return 'w';
  return null;
}

export function resolveAcquisitionsRightsFromProfile(
  user: Pick<User, 'acquisitionsRights' | 'rights'> | null | undefined,
): AccountTypeRightLevel | null {
  if (!user) return null;
  const nested = user.rights && typeof user.rights === 'object' ? user.rights : null;
  return normalizeAcquisitionsRightsLevel(nested?.acquisitionsRights ?? user.acquisitionsRights);
}

/**
 * Effective acquisitions permission: JWT `rights.acquisitionsRights` first,
 * then GET /auth/me fields when present.
 */
export function resolveAcquisitionsRights(
  user: Pick<User, 'acquisitionsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): AccountTypeRightLevel | null {
  const fromJwt = authToken ? readAcquisitionsRightsFromJwt(authToken) : null;
  if (fromJwt) return fromJwt;
  return resolveAcquisitionsRightsFromProfile(user);
}

/** Staff acquisitions list/detail (`r` or `w`). Absent claim is deny — distinct from cataloging. */
export const canViewAcquisitions = (
  user: Pick<User, 'accountType' | 'acquisitionsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean => {
  const level = resolveAcquisitionsRights(user, authToken);
  return level === 'r' || level === 'w';
};

/** Mutations (vendors/funds/orders/receipt) require write. Absent claim is deny. */
export const canManageAcquisitions = (
  user: Pick<User, 'accountType' | 'acquisitionsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean => {
  return resolveAcquisitionsRights(user, authToken) === 'w';
};

type WeedingRightUser = Pick<
  User,
  'weedingRights' | 'weedingCancelRights' | 'itemsArchiveRights' | 'itemsRights' | 'rights'
> | null | undefined;

function resolveNamedRight(
  user: WeedingRightUser,
  authToken: string | null | undefined,
  profileKey: 'weedingRights' | 'weedingCancelRights' | 'itemsArchiveRights' | 'itemsRights',
  fromJwt: (token: string | null | undefined) => AccountTypeRightLevel | null,
): AccountTypeRightLevel | null {
  const jwt = authToken ? fromJwt(authToken) : null;
  if (jwt) return jwt;
  if (!user) return null;
  const nested = user.rights && typeof user.rights === 'object' ? user.rights : null;
  return normalizeAcquisitionsRightsLevel(nested?.[profileKey] ?? user[profileKey]);
}

/** Staff weeding list and CSV (`r` or `w`). A missing claim is deny. */
export const canViewWeeding = (user: WeedingRightUser, authToken?: string | null): boolean => {
  const level = resolveNamedRight(user, authToken, 'weedingRights', readWeedingRightsFromJwt);
  return level === 'r' || level === 'w';
};

/** Withdraw, batch withdraw, legacy completion, and candidate / on-shelf weeding. */
export const canManageWeeding = (user: WeedingRightUser, authToken?: string | null): boolean => {
  return resolveNamedRight(user, authToken, 'weedingRights', readWeedingRightsFromJwt) === 'w';
};

/** Cancel an active, not-yet-disposed withdrawal. */
export const canCancelWeeding = (user: WeedingRightUser, authToken?: string | null): boolean => {
  return resolveNamedRight(user, authToken, 'weedingCancelRights', readWeedingCancelRightsFromJwt) === 'w';
};

/** `DELETE /items/{id}`. Separate from weeding. */
export const canArchiveItems = (user: WeedingRightUser, authToken?: string | null): boolean => {
  return resolveNamedRight(user, authToken, 'itemsArchiveRights', readItemsArchiveRightsFromJwt) === 'w';
};

/** `DELETE /biblios/{id}` still requires `itemsRights = w`. */
export const canDeleteBiblios = (user: WeedingRightUser, authToken?: string | null): boolean => {
  return resolveNamedRight(user, authToken, 'itemsRights', readItemsRightsFromJwt) === 'w';
};

/** PUT /settings/email-templates/:templateId/:language */
export interface UpdateEmailTemplateRequest {
  subject: string;
  bodyPlain: string;
  bodyHtml: string | null;
}

/** Row from GET /settings/email-templates */
export interface EmailTemplateListItem {
  templateId: string;
  language: string;
  /** Localized staff-facing label for this (templateId, language) row (read-only). */
  name: string;
  subject?: string | null;
  bodyPlain?: string | null;
  bodyHtml?: string | null;
  updatedAt?: string | null;
}

export type EmailTemplateDetail = EmailTemplateListItem & {
  subject: string;
  bodyPlain: string;
  bodyHtml: string | null;
};

// Admin dynamic config (GET/PUT/DELETE /admin/config)
export type AdminConfigSectionKey =
  | 'email'
  | 'logging'
  | 'reminders'
  | 'audit'
  | 'holds'
  | 'circulation'
  | 'features';

export type LogRotation = 'daily' | 'weekly' | 'monthly' | 'never';

export interface LoggingConfig {
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error';
  format: 'pretty' | 'plain' | 'json';
  output: 'stdout' | 'stderr' | 'file' | 'syslog';
  file_path?: string | null;
  file_rotation?: LogRotation | null;
  overridable?: boolean;
}

export const LOG_ROTATION_OPTIONS: LogRotation[] = ['daily', 'weekly', 'monthly', 'never'];

export interface ConfigSectionInfo {
  key: string;
  value: Record<string, unknown>;
  overridden: boolean;
  overridable: boolean;
}

export interface AdminConfigResponse {
  sections: ConfigSectionInfo[];
}

/** Graduated overdue notice: 1st, 2nd, then formal notice (mise en demeure). */
export type ReminderTierId = 'first' | 'second' | 'formalNotice';

/** GET/PUT /admin/config section `reminders` (wire format is snake_case). */
export interface RemindersConfig {
  enabled: boolean;
  frequencyDays: number;
  firstReminderDelayDays: number;
  secondReminderDelayDays: number;
  formalNoticeDelayDays: number;
  firstReminderTemplate: string;
  secondReminderTemplate: string;
  formalNoticeTemplate: string;
  firstReminderEnabled: boolean;
  secondReminderEnabled: boolean;
  formalNoticeEnabled: boolean;
  sendTime: string;
  accrueFines: boolean;
  smtpThrottleMs: number;
}

/** POST /admin/reindex-search */
export interface ReindexSearchResponse {
  itemsQueued: number;
  meilisearchAvailable: boolean;
}

export type MaintenanceAction =
  | 'cleanupDanglingBiblioSeries'
  | 'cleanupDanglingBiblioCollections'
  | 'cleanupSeries'
  | 'cleanupCollections'
  | 'mergeDuplicateSeries'
  | 'mergeDuplicateCollections'
  | 'cleanupOrphanAuthors'
  | 'cleanupUsers';

/** POST /maintenance — tagged action for Z39.50 catalog refresh (requires server id). */
export interface Z3950RefreshMaintenanceAction {
  action: 'z3950Refresh';
  z3950ServerId: number;
  rebuildAll?: boolean;
}

export type MaintenanceRequestAction = MaintenanceAction | Z3950RefreshMaintenanceAction;

/** Summary in maintenance report `details` for z3950Refresh (camelCase from API). */
export interface CatalogZ3950RefreshResult {
  z3950ServerId: number;
  rebuildAll: boolean;
  total: number;
  updated: number;
  notFound: number;
  failed: number;
}

export interface MaintenanceActionReport {
  action: MaintenanceRequestAction | string;
  success: boolean;
  details: Record<string, number> | CatalogZ3950RefreshResult;
  error?: string;
}

export interface MaintenanceResponse {
  reports: MaintenanceActionReport[];
}

// Audit log
export interface AuditLogEntry {
  id: number;
  eventType: string;
  /** Defaults to success for legacy rows without migration fields */
  outcome?: 'success' | 'failure';
  userId: number | null;
  entityType: string | null;
  entityId: number | null;
  ipAddress: string | null;
  payload: Record<string, unknown> | null;
  httpStatus?: number | null;
  errorCode?: string | null;
  errorMessage?: string | null;
  createdAt: string;
}

export interface AuditLogPage {
  entries: AuditLogEntry[];
  total: number;
  page: number;
  perPage: number;
}

/** `GET /loans/overdue` borrower when the caller can read users. */
export interface OverdueBorrowerFull {
  id: string;
  firstname?: string | null;
  lastname?: string | null;
  email?: string | null;
  phone?: string | null;
}

/** `GET /loans/overdue` borrower without users read rights. */
export interface OverdueBorrowerMinimal {
  id: string;
  cardNumber?: string | null;
  initials?: string | null;
}

export type OverdueBorrower = OverdueBorrowerFull | OverdueBorrowerMinimal;

// Overdue loans dashboard
export interface OverdueLoanInfo {
  loanId: string;
  user: OverdueBorrower;
  biblioId?: string;
  title?: string;
  authors?: string;
  itemBarcode?: string;
  loanDate: string;
  expiryAt: string | null;
  lastReminderSentAt: string | null;
  reminderCount: number;
  nextReminderTier?: string | null;
}

export interface OverdueLoansPage {
  loans: OverdueLoanInfo[];
  total: number;
  page: number;
  perPage: number;
}

export interface ReminderDetail {
  userId: string;
  email: string;
  firstname?: string;
  lastname?: string;
  loanCount: number;
}

export interface ReminderError {
  userId: string;
  email: string;
  errorMessage: string;
}

export interface ReminderReport {
  dryRun: boolean;
  emailsSent: number;
  loansReminded: number;
  details: ReminderDetail[];
  errors: ReminderError[];
}

// Events
export interface EventAttachmentInput {
  fileName: string;
  mimeType: string;
  dataBase64: string;
}

export interface Event {
  id: string;
  name: string;
  eventType: number;
  eventDate: string;
  startTime?: string | null;
  endTime?: string | null;
  description?: string | null;
  partnerName?: string | null;
  schoolName?: string | null;
  className?: string | null;
  attendeesCount?: number | null;
  studentsCount?: number | null;
  /** True when the event targets every audience. Mutually exclusive with `publicTypes`. */
  allAudiences?: boolean;
  /** Selected audience names (`public_types.name`). Empty when `allAudiences` is true. */
  publicTypes?: string[];
  /** @deprecated Legacy single audience. Prefer `publicTypes`. */
  publicType?: string | null;
  notes?: string | null;
  announcementSentAt?: string | null;
  createdAt?: string | null;
  updateAt?: string | null;
  /** Present on list responses when a file is stored (no Base64 on list). */
  attachmentFileName?: string | null;
  attachmentMimeType?: string | null;
  attachmentSize?: number | null;
  /** Present on GET single / POST / PUT responses when a file is stored. */
  attachmentDataBase64?: string | null;
}

/** `GET /events/{id}/announcement-recipients/count` — opted-in recipients only. */
export interface AnnouncementRecipientCount {
  count: number;
}

export interface CreateEvent {
  name: string;
  eventDate: string;
  eventType?: number | null;
  startTime?: string | null;
  endTime?: string | null;
  description?: string | null;
  partnerName?: string | null;
  schoolName?: string | null;
  className?: string | null;
  attendeesCount?: number | null;
  studentsCount?: number | null;
  allAudiences?: boolean;
  publicTypes?: string[];
  /** @deprecated Send `publicTypes` instead. Mapped server-side to a one-element list. */
  publicType?: string | null;
  notes?: string | null;
  attachment?: EventAttachmentInput | null;
}

export interface UpdateEvent {
  name?: string | null;
  eventDate?: string | null;
  eventType?: number | null;
  startTime?: string | null;
  endTime?: string | null;
  description?: string | null;
  partnerName?: string | null;
  schoolName?: string | null;
  className?: string | null;
  attendeesCount?: number | null;
  studentsCount?: number | null;
  allAudiences?: boolean | null;
  publicTypes?: string[] | null;
  /** @deprecated Send `publicTypes` instead. */
  publicType?: string | null;
  notes?: string | null;
  attachment?: EventAttachmentInput | null;
  removeAttachment?: boolean | null;
}

export interface EventsListResponse {
  events: Event[];
  total: number;
}

// ──────────────────────────────────────────────────────────────────
// Holds (copy-level and title-level) — API tag `holds`
// ──────────────────────────────────────────────────────────────────

export type HoldStatus = 'pending' | 'to_pick' | 'ready' | 'fulfilled' | 'cancelled' | 'expired';

export type MyHoldStatus = HoldStatus | 'inTransit';

/**
 * `GET /me/holds` — the caller's own holds (`MyHoldView`).
 * Does not embed a catalog biblio or copy.
 * `patronNote` is the reader's remark. `internalNote` is absent.
 * Null or absent after the hold is closed or the account is anonymized.
 * `pickupDeadline` is set only when `status` is `ready`.
 * `inTransit` is set when an active transit is linked to the hold.
 */
export interface MyHold {
  id: string;
  biblioId: string;
  title: string | null;
  status: MyHoldStatus;
  pickupDeadline: string | null;
  pickupSiteId: string | null;
  pickupSiteName: string | null;
  position: number;
  createdAt: string;
  patronNote?: string | null;
}

/**
 * `GET /holds` staff search row (`HoldListStaff`).
 * Both notes are omitted. Staff queues use {@link HoldDetailsStaff}.
 * `biblio.items` is empty for an unassigned title-level hold; otherwise one {@link ItemShortStaff}.
 */
export interface HoldListStaff {
  id: string;
  userId: string;
  biblioId: string;
  /** Null while a title-level hold is waiting for a copy. */
  itemId: string | null;
  pickupSiteId: string | null;
  biblio: BiblioShortStaff;
  /**
   * Users read: {@link UserShortStaff}. Every other caller allowed to see the
   * row: {@link LoanBorrowerMinimized} (card number or initials, never the name).
   */
  user: HoldFor | null;
  createdAt: string;
  notifiedAt: string | null;
  expiresAt: string | null;
  status: HoldStatus;
  position: number;
}

/**
 * Staff queue row (`GET /items/:id/holds`, `GET /biblios/:id/holds`, `GET /users/:id/holds`).
 * Both notes are present for holds read or write. Null when unset, and cleared
 * when the hold is closed or the reader is anonymized. Absent on search rows.
 */
export interface HoldDetailsStaff extends HoldListStaff {
  patronNote?: string | null;
  internalNote?: string | null;
}

/**
 * Public `GET /holds` row (`HoldDetailsPublic`).
 * Neither note is included. Embedded copies are {@link BiblioShortPublic}.
 */
export interface HoldDetailsPublic {
  id: string;
  userId: string;
  biblioId: string;
  itemId: string | null;
  pickupSiteId: string | null;
  biblio: BiblioShortPublic;
  user: UserShortPublic | null;
  createdAt: string;
  notifiedAt: string | null;
  expiresAt: string | null;
  status: HoldStatus;
  position: number;
}

/** `GET /holds` — staff search (`HoldListStaff`) or a public list (`HoldDetailsPublic`). */
export type HoldListItem = HoldListStaff | HoldDetailsPublic;

/** Staff queue row. Same JSON as {@link HoldDetailsStaff}. */
export type Hold = HoldDetailsStaff;

/**
 * `POST /holds`. Do not send `notes` (400 `validation_error`).
 * Omit a note that did not change. `""` clears it.
 * `patronNote` is for the reader on their own hold, and for holds write.
 * `internalNote` requires holds write; any other caller gets 403 `authorization_failed`.
 */
export interface CreateHold {
  userId: string;
  /** Pinned copy. Omit for a title-level hold. */
  itemId?: string;
  /** Always sent. Required when `itemId` is omitted. */
  biblioId?: string;
  pickupSiteId?: string | null;
  patronNote?: string;
  internalNote?: string;
  force?: boolean;
}

/**
 * One shelf row from `GET /holds/pick-list`.
 * `holdFor` is always `{ id, cardNumber, initials }`. The name is never stored:
 * these labels sit on an open shelf and are printed.
 */
export interface HoldPickListRow {
  holdId: string;
  itemId: string;
  barcode: string | null;
  callNumber: string | null;
  title: string | null;
  userId: string;
  holdFor: LoanBorrowerMinimized;
  pickupSiteId: string | null;
  pickupSiteName: string | null;
  pickStartedAt: string | null;
  pickDueAt: string | null;
  /** `holds.expires_at`. Null while the hold is still `to_pick`. */
  pickupDeadline: string | null;
  overdue: boolean;
}

/** Desk view of a replacement charge on a lost copy (`pending` is reported as `unpaid`). */
export type LostFineDeskStatus = 'unpaid' | 'paid' | 'partial';

/**
 * Optional fields on a return when the copy's status is lost.
 * Omitted on a normal return. Amounts are cents. Absent fine fields mean no open charge.
 */
export interface LostReturnFields {
  wasLost?: boolean;
  fineId?: string;
  fineStatus?: LostFineDeskStatus;
  fineAmount?: number;
  paidAmount?: number;
}

/** Non-blocking note on a barcode check-in. Not an error body. */
export type DeskNoticeCode = 'item_not_on_loan' | 'item_not_available_status';

/** `informational` is neutral. `warning` means the copy was not changed. */
export type DeskNoticeLevel = 'informational' | 'warning';

export interface DeskNotice {
  code: DeskNoticeCode;
  level: DeskNoticeLevel;
}

/**
 * camelCase JSON from `CirculationStatus` on a blocked return scan.
 * `available` is sent with `weedingStatus` `withdrawn`.
 */
export type ReturnScanCirculationStatus =
  | 'available'
  | 'lost'
  | 'damaged'
  | 'claimedReturned'
  | 'toSearch'
  | 'missing';

/** Desk check-in of a barcode (`POST /loans/items/{barcode}/return`). */
export interface ItemCheckIn extends LostReturnFields {
  status: string;
  loan?: Loan | null;
  hold?: Hold | null;
  userId?: string | null;
  userName?: string | null;
  patronName?: string | null;
  destinationSiteName?: string | null;
  pickupSiteName?: string | null;
  pickupSiteId?: string | null;
  destinationSiteId?: string | null;
  transitStarted?: boolean;
  /**
   * Available copy that was not on loan (`item_not_on_loan`, `informational`),
   * or a blocked status (`item_not_available_status`, `warning`).
   */
  notice?: DeskNotice | null;
  /** Patron a not-on-loan hold was set aside for. Same privacy rule as catalog `holdFor`. */
  holdFor?: HoldFor | null;
  /** Current circulation exception when the notice is `item_not_available_status`. */
  circulationStatus?: ReturnScanCirculationStatus | null;
  /** Current weeding status when the notice is `item_not_available_status`. */
  weedingStatus?: ItemWeedingStatus | null;
  /**
   * True when this check-in returned a deposit copy whose batch is `returning`.
   * Staff send that copy back to the lender. False on every other check-in.
   */
  returnToLender?: boolean;
  /**
   * Set-aside only. `ready` when the copy is already at the pickup site,
   * `pending` while it is in transit.
   */
  holdStatus?: 'ready' | 'pending' | null;
  /**
   * Set-aside only. Null when the copy became ready on site.
   * A string when a transit was started or reused.
   */
  transitId?: string | null;
}

/** Body for `POST /loans/items/{barcode}/return` when setting a title hold aside. */
export interface ShelfSetAsideRequest {
  holdId: string;
  itemId: string;
}

/** Staff decision after the lost-copy alert (`POST /loans/items/{itemId}/lost-found`). */
export type LostFoundDisposition = 'circulate' | 'damaged' | 'leave';

export type LostFineAction = 'none' | 'cancelled' | 'refunded' | 'partial' | 'unchanged';

export interface ResolveLostFoundRequest {
  disposition: LostFoundDisposition;
  notes?: string;
}

/** Amounts are cents. `hold` is set only when a pending hold was readied. */
export interface LostFoundResolution {
  itemId: string;
  itemStatus: string;
  borrowable: boolean;
  disposition: LostFoundDisposition;
  fineAction: LostFineAction;
  fineId?: string;
  refundedAmount: number;
  cancelledAmount: number;
  hold?: Hold | null;
}

/** GET/PUT /holds/policy — global max pending+ready holds (camelCase). */
export interface HoldsPolicy {
  maxActiveHolds: number;
}

/** GET /holds/quota — resolved cap and remaining slots for a patron. */
export interface HoldQuota {
  userId: string;
  maxActiveHolds: number;
  activeHolds: number;
  remaining: number;
}

// ──────────────────────────────────────────────────────────────────
// Inter-site item transit (hold fulfillment) — API tag `transits`
// ──────────────────────────────────────────────────────────────────

/** camelCase wire values from #40 (`inTransit`, not `in_transit`). */
export type TransitStatus = 'requested' | 'inTransit' | 'received' | 'cancelled';

/**
 * Reader on a transit. Card number and initials only — never a name.
 * `initials` is shown as returned (`JD` from the server, or a dotted form such as `J. D.`).
 * Null when the transit has no linked hold, the linked hold is not pending, set aside,
 * or ready, or the caller cannot see hold patrons. Omitted by servers that do not send it yet.
 */
export interface TransitPatron {
  cardNumber?: string | null;
  initials?: string | null;
}

export interface ItemTransit {
  id: string;
  itemId: string;
  holdId?: string | null;
  fromSiteId: string;
  toSiteId: string;
  status: TransitStatus;
  notes?: string | null;
  createdAt: string;
  shippedAt?: string | null;
  receivedAt?: string | null;
  cancelledAt?: string | null;
  shippedBy?: string | null;
  receivedBy?: string | null;
  cancelledBy?: string | null;
  reversedFromId?: string | null;
  /**
   * Copy and patron fields. Optional so a server that does not send them yet
   * still type-checks. `siteId` is the copy's current site.
   */
  barcode?: string | null;
  title?: string | null;
  siteId?: string | null;
  siteName?: string | null;
  patron?: TransitPatron | null;
}

/** POST /holds/{id}/transits — allocate a copy toward the hold’s pickup site. */
export interface CreateTransit {
  itemId?: string | null;
  fromSiteId?: string | null;
  /** Must match hold.pickupSiteId when that field is already set. */
  toSiteId?: string | null;
  notes?: string | null;
  /** Skip `requested` and mark the copy in transit immediately. */
  ship?: boolean;
}

/** POST /transits/{id}/ship|receive|cancel */
export interface TransitActionRequest {
  notes?: string | null;
  /** On cancel: open a reverse transit back to the origin. */
  reverse?: boolean | null;
}

export interface TransitWithHold {
  transit: ItemTransit;
  hold?: Hold | null;
}

export interface ListTransitsParams {
  status?: TransitStatus;
  itemId?: string;
  holdId?: string;
  toSiteId?: string;
  page?: number;
  perPage?: number;
}

// ──────────────────────────────────────────────────────────────────
// Fines / Penalties
// ──────────────────────────────────────────────────────────────────

/** Wire values from `FineStatus`. Includes lost-copy settlements. */
export type FineStatus = 'pending' | 'partial' | 'paid' | 'waived' | 'cancelled' | 'refunded';

export type FineChargeType = 'overdue' | 'replacement' | 'damage';

/**
 * One row from `GET /users/{id}/fines`.
 * Amounts are decimal euro strings (`"12.50"`), not cents.
 * `refundedAmount` is omitted when it is zero.
 */
export interface Fine {
  id: string;
  loanId?: string;
  userId?: string;
  amount: string;
  paidAmount?: string;
  refundedAmount?: string;
  status: FineStatus;
  createdAt?: string;
  paidAt?: string | null;
  notes?: string | null;
  chargeType?: FineChargeType;
  itemId?: string;
}

export interface FinesResponse {
  /** Decimal euro string. Open charges only (`pending` and `partial`). */
  totalUnpaid?: string;
  fines: Fine[];
}

export interface FineRule {
  id?: number;
  mediaType?: MediaType | null;
  dailyRate?: string;
  maxAmount?: string;
  graceDays?: number;
  notes?: string | null;
}

export type GlobalUnpaidThresholdMode = 'none' | 'amount';
export type AudienceUnpaidThresholdMode = 'inherit' | 'none' | 'amount';

/** GET/PUT /fines/policy — global unpaid-fine gate (camelCase). */
export interface CirculationFinePolicy {
  unpaidThresholdMode: GlobalUnpaidThresholdMode;
  /** Integer cents. Present only when `unpaidThresholdMode` is `amount`. */
  unpaidThresholdCents?: number | null;
}

export type AccrueOutcome =
  | 'created'
  | 'updated'
  | 'unchanged'
  | 'skippedGrace'
  | 'skippedNotOverdue'
  | 'skippedReturned'
  | 'skippedDeletedUser';

export interface AccrualBreakdown {
  overdueDays: number;
  graceDays: number;
  billableDays: number;
  dailyRate: string;
  maxAmount?: string | null;
  amount: string;
  capped: boolean;
  mediaType?: string | null;
}

export interface AccrueLoanResult {
  outcome: AccrueOutcome;
  fine?: Fine | null;
  breakdown?: AccrualBreakdown | null;
  loanId: string;
  userId: string;
}

export interface AccrueBatchError {
  loanId: string;
  errorMessage: string;
}

export interface AccrueBatchReport {
  created: number;
  updated: number;
  unchanged: number;
  skipped: number;
  errors: AccrueBatchError[];
  results: AccrueLoanResult[];
}

// ──────────────────────────────────────────────────────────────────
// Inventory / Stock check
// ──────────────────────────────────────────────────────────────────

export type InventoryScanResultCode =
  | 'found'
  | 'found_out_of_scope'
  | 'found_archived'
  | 'unknown_barcode';

export interface InventorySession {
  id: string;
  /** Session label (required on create; may be absent on legacy rows) */
  name?: string;
  locationFilter?: string | null;
  status: 'open' | 'closed';
  startedAt?: string | null;
  /** @deprecated prefer startedAt */
  createdAt?: string;
  closedAt?: string | null;
  notes?: string | null;
  createdBy?: string | null;
  /** When set, scope is active non-archived items with this `items.place` only; null = entire active collection */
  scopePlace?: number | null;
  /** Catalog source filter (`items.site_id`); null = all sites */
  scopeSiteId?: string | null;
  scopeSiteName?: string | null;
  consolidatedAt?: string | null;
  consolidatedBy?: string | null;
}

export interface CreateInventorySession {
  name: string;
  locationFilter?: string | null;
  notes?: string | null;
  scopePlace?: number | null;
  scopeSiteId?: string | null;
}

export interface CreateInventorySessionResponse {
  session: InventorySession;
  expectedInScope: number;
  warnings: string[];
}

export interface InventoryScan {
  id: number;
  sessionId: string;
  barcode: string;
  itemId?: string | null;
  scannedAt?: string | null;
  result: InventoryScanResultCode;
  scannedBy?: string | null;
}

/** @deprecated use InventoryScan */
export type InventoryScanResult = InventoryScan;

export interface InventoryMissingRow {
  itemId: string;
  barcode?: string | null;
  callNumber?: string | null;
  place?: number | null;
  biblioTitle?: string | null;
  siteId?: string | null;
  siteName?: string | null;
}

export interface InventoryReport {
  sessionId: string;
  expectedInScope?: number;
  totalScanned?: number;
  totalFound?: number;
  totalFoundArchived?: number;
  totalFoundOutOfScope?: number;
  totalUnknown?: number;
  distinctItemsScanned?: number;
  duplicateScanCount?: number;
  missingCount?: number;
  missingScannable?: number;
  missingWithoutBarcode?: number;
}

export interface InventoryConsolidationPreviewSummary {
  totalMissing: number;
  onLoanCount: number;
  deletableWithoutForce: number;
  orphanBibliosCount: number;
  affectedReadersCount: number;
}

export interface InventoryConsolidationPreviewLoan {
  loanId: string;
  userId: string;
  userEmail?: string | null;
  userFirstname?: string | null;
  userLastname?: string | null;
  expiryAt?: string | null;
}

export interface InventoryConsolidationPreviewRow {
  itemId: string;
  barcode?: string | null;
  callNumber?: string | null;
  place?: number | null;
  siteId?: string | null;
  siteName?: string | null;
  biblioId?: string | null;
  biblioTitle?: string | null;
  onLoan: boolean;
  wouldSkipWithoutForce: boolean;
  biblioWouldBeOrphaned: boolean;
  activeLoan?: InventoryConsolidationPreviewLoan | null;
}

export interface InventoryConsolidationPreview {
  sessionId: string;
  summary: InventoryConsolidationPreviewSummary;
  items: InventoryConsolidationPreviewRow[];
  total: number;
  page: number;
  perPage: number;
  pageCount: number;
}

export interface ConsolidateInventorySession {
  force?: boolean;
}

export interface InventoryConsolidationSkipped {
  itemId: string;
  reason: string;
}

export interface InventoryConsolidationEmailError {
  userId: string;
  email: string;
  errorMessage: string;
}

export interface InventoryConsolidationResult {
  sessionId: string;
  attempted: number;
  deleted: number;
  skipped: InventoryConsolidationSkipped[];
  consolidated: boolean;
  archivedBiblios: number;
  loanClosureEmailsSent: number;
  loanClosureEmailErrors: InventoryConsolidationEmailError[];
}

// ──────────────────────────────────────────────────────────────────
// Reading history (RGPD)
// ──────────────────────────────────────────────────────────────────

export interface HistoryPreference {
  userId?: string;
  historyEnabled?: boolean;
}

export interface ReadingHistoryEntry {
  id: string;
  loanId?: string | null;
  biblio?: BiblioShort | null;
  returnedAt?: string;
}

// ──────────────────────────────────────────────────────────────────
// Batch operations (scanner / return kiosk)
// ──────────────────────────────────────────────────────────────────

export interface BatchReturnResult extends LostReturnFields {
  barcode: string;
  success: boolean;
  loan?: Loan | null;
  error?: string | null;
  notice?: DeskNotice | null;
  circulationStatus?: ReturnScanCirculationStatus | null;
  weedingStatus?: ItemWeedingStatus | null;
}

export interface BatchReturnResponse {
  returned: number;
  errors: number;
  results: BatchReturnResult[];
}

export interface BatchCreateResult {
  barcode: string;
  success: boolean;
  loanId?: string | null;
  /** Stable API code. Absent on older servers, which send only `error`. */
  code?: string | null;
  /** English server text. Shown only when `code` is missing. */
  error?: string | null;
  /** Server-computed. Missing means the row is not forceable. */
  forceable?: boolean | null;
  /**
   * Server `reasonRequired`. Batch checkout cannot send `overrideReason`,
   * so a true flag offers no force button and no reason field.
   */
  reasonRequired?: boolean | null;
  /** Present on `item_status_action_required` (`lost`, `claimedReturned`, `toSearch`, `missing`). */
  itemStatus?: string | null;
  /** Oldest pending hold id, when this row includes it. */
  holdId?: string | null;
  /** Scanned copy id, when this row includes it. */
  itemId?: string | null;
}

export interface BatchCreateResponse {
  created: number;
  errors: number;
  results: BatchCreateResult[];
}

// ──────────────────────────────────────────────────────────────────
// OPAC — public unauthenticated catalogue
// ──────────────────────────────────────────────────────────────────

export interface OPACAvailability {
  biblioId?: string;
  activeLoans?: number;
  holdCount?: number;
}

// ──────────────────────────────────────────────────────────────────
// First setup (GET /health, POST /first_setup)
// ──────────────────────────────────────────────────────────────────

export interface HealthSetupInfo {
  needFirstSetup?: boolean;
  need_first_setup?: boolean;
}

export interface HealthResponse {
  status: string;
  version?: string;
  database?: { connected?: boolean };
  setup?: HealthSetupInfo;
}

/** True when the server requires the initial wizard (no users / no settings yet). */
export function healthNeedsFirstSetup(h: HealthResponse | undefined | null): boolean {
  if (!h) return false;
  if (h.status === 'need_first_setup') return true;
  const s = h.setup;
  if (!s) return false;
  return Boolean(s.needFirstSetup ?? s.need_first_setup);
}

/** True when the app cannot operate normally (e.g. DB unreachable — GET /health still 200). */
export function healthIsDegraded(h: HealthResponse | undefined | null): boolean {
  if (!h) return false;
  if (h.status === 'degraded') return true;
  if (h.database?.connected === false) return true;
  return false;
}

export interface FirstSetupAdmin {
  login: string;
  password: string;
  firstname: string;
  lastname: string;
  sex: 'm' | 'f';
  birthdate: string;
  email?: string;
  language?: string;
}

export interface FirstSetupEmailOverride {
  smtpHost?: string;
  smtpPort?: number;
  smtpUsername?: string;
  smtpPassword?: string;
  smtpFrom?: string;
  smtpFromName?: string;
  smtpUseTls?: boolean;
  templatesDir?: string;
}

export interface FirstSetupRequest {
  admin: FirstSetupAdmin;
  library: UpdateLibraryInfoRequest;
  email?: FirstSetupEmailOverride;
}

export interface FirstSetupResponse {
  token: string;
  tokenType: string;
  expiresIn: number;
  user: LoginResponse['user'];
  libraryInfo: LibraryInfo;
}

// ──────────────────────────────────────────────────────────────────
// Acquisitions (vendors, yearly funds, purchase orders, receipt)
// ──────────────────────────────────────────────────────────────────

export type PurchaseOrderStatus = 'draft' | 'ordered' | 'partial' | 'received' | 'cancelled';

/** rust_decimal may serialize as a number or a string. */
export type MoneyAmount = string | number;

export interface AcquisitionVendor {
  id: string;
  name: string;
  code?: string | null;
  email?: string | null;
  phone?: string | null;
  address?: string | null;
  notes?: string | null;
  active: boolean;
  createdAt: string;
  updatedAt: string;
  archivedAt?: string | null;
}

export interface CreateVendor {
  name: string;
  code?: string | null;
  email?: string | null;
  phone?: string | null;
  address?: string | null;
  notes?: string | null;
  active?: boolean | null;
}

export interface UpdateVendor {
  name?: string | null;
  code?: string | null;
  email?: string | null;
  phone?: string | null;
  address?: string | null;
  notes?: string | null;
  active?: boolean | null;
}

export interface VendorListResponse {
  vendors: AcquisitionVendor[];
  total: number;
}

export interface AcquisitionFund {
  id: string;
  code: string;
  name: string;
  fiscalYear: number;
  allocatedAmount: MoneyAmount;
  currency: string;
  notes?: string | null;
  createdAt: string;
  updatedAt: string;
  committed: MoneyAmount;
  spent: MoneyAmount;
  available: MoneyAmount;
}

export interface CreateFund {
  code: string;
  name: string;
  fiscalYear: number;
  allocatedAmount?: MoneyAmount | null;
  currency?: string | null;
  notes?: string | null;
}

export interface UpdateFund {
  code?: string | null;
  name?: string | null;
  fiscalYear?: number | null;
  allocatedAmount?: MoneyAmount | null;
  currency?: string | null;
  notes?: string | null;
}

export interface FundListResponse {
  funds: AcquisitionFund[];
  total: number;
}

export interface PurchaseOrder {
  id: string;
  vendorId: string;
  fundId?: string | null;
  orderNumber: string;
  status: PurchaseOrderStatus;
  notes?: string | null;
  orderedAt?: string | null;
  createdBy?: string | null;
  createdAt: string;
  updatedAt: string;
  vendorName?: string | null;
  fundCode?: string | null;
}

export interface PurchaseOrderLine {
  id: string;
  purchaseOrderId: string;
  fundId?: string | null;
  biblioId?: string | null;
  isbn?: string | null;
  title?: string | null;
  quantityOrdered: number;
  quantityReceived: number;
  unitPrice?: MoneyAmount | null;
  currency: string;
  notes?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface PurchaseOrderDetail {
  order: PurchaseOrder;
  lines: PurchaseOrderLine[];
}

export interface CreateOrderLine {
  fundId?: string | null;
  biblioId?: string | null;
  isbn?: string | null;
  title?: string | null;
  quantity: number;
  unitPrice?: MoneyAmount | null;
  currency?: string | null;
  notes?: string | null;
}

export interface UpdateOrderLine {
  fundId?: string | null;
  biblioId?: string | null;
  isbn?: string | null;
  title?: string | null;
  quantity?: number | null;
  unitPrice?: MoneyAmount | null;
  currency?: string | null;
  notes?: string | null;
}

export interface CreatePurchaseOrder {
  vendorId: string;
  fundId?: string | null;
  orderNumber?: string | null;
  notes?: string | null;
  lines?: CreateOrderLine[] | null;
}

export interface UpdatePurchaseOrder {
  vendorId?: string | null;
  fundId?: string | null;
  orderNumber?: string | null;
  notes?: string | null;
}

export interface ReceiveItemSpec {
  barcode?: string | null;
  siteId?: string | null;
  siteName?: string | null;
  price?: string | null;
  /** Staff explicitly defers recording a price (report later). */
  priceDeferred?: boolean;
  callNumber?: string | null;
}

export interface ReceiveOrderLine {
  lineId: string;
  quantity: number;
  unitPrice?: MoneyAmount | null;
  items?: ReceiveItemSpec[] | null;
}

export interface ReceivePurchaseOrder {
  notes?: string | null;
  lines: ReceiveOrderLine[];
}

export interface Receipt {
  id: string;
  purchaseOrderId: string;
  receivedAt: string;
  receivedBy?: string | null;
  notes?: string | null;
  createdAt: string;
}

export interface ReceiptLineResult {
  id: string;
  purchaseOrderLineId: string;
  quantity: number;
  unitPrice?: MoneyAmount | null;
  itemIds: string[];
}

export interface ReceivePurchaseOrderResult {
  receipt: Receipt;
  lines: ReceiptLineResult[];
  order: PurchaseOrderDetail;
}

export interface PurchaseOrderListResponse {
  orders: PurchaseOrder[];
  total: number;
}

export type PurchaseSuggestionStatus = 'proposed' | 'accepted' | 'refused';

/** Patron title proposal. Accept opens a draft purchase-order intent (no biblio, no item). */
export interface PurchaseSuggestion {
  id: string;
  proposedBy: string;
  proposedByName?: string | null;
  title: string;
  author: string;
  comment?: string | null;
  status: PurchaseSuggestionStatus;
  staffNote?: string | null;
  reviewedBy?: string | null;
  reviewedAt?: string | null;
  purchaseOrderId?: string | null;
  purchaseOrderLineId?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface CreatePurchaseSuggestion {
  title: string;
  author: string;
  comment?: string | null;
}

export interface ReviewPurchaseSuggestion {
  staffNote?: string | null;
}

export interface PurchaseSuggestionListResponse {
  suggestions: PurchaseSuggestion[];
  total: number;
}
