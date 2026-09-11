import {
  hasMinRightsLevel,
  normalizeRightsLevel,
  readAcquisitionsRightsFromJwt,
  readDomainRightFromJwt,
  type RightsDomain,
  type RightsLevel,
} from '@/utils/jwtRights';

export type { RightsDomain, RightsLevel };
export { hasMinRightsLevel, normalizeRightsLevel, readDomainRightFromJwt };

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
  updatedAt?: string | null;
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
  itemsRights?: string | null;
  usersRights?: string | null;
  loansRights?: string | null;
  holdsRights?: string | null;
  /** Legacy alias on some tokens — prefer holdsRights */
  borrowsRights?: string | null;
  settingsRights?: string | null;
  eventsRights?: string | null;
  /** Acquisitions (`n`/`r`/`w` or `none`/`read`/`write`) — distinct from cataloging. */
  acquisitionsRights?: string | null;
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
  itemsRights?: string | null;
  usersRights?: string | null;
  loansRights?: string | null;
  settingsRights?: string | null;
  eventsRights?: string | null;
  /** Acquisitions permission when exposed on the profile (`n`/`r`/`w` or `none`/`read`/`write`). */
  acquisitionsRights?: string | null;
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
  publicType?: string | null;
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
}

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
}

/** First-login / forced password change via POST /auth/change-password */
export interface ChangePasswordRequest {
  newPassword: string;
}

/** POST /auth/request-password-reset (public, no JWT) */
export interface RequestPasswordResetRequest {
  identifier: string;
  /** Full URL template; must contain literal `<token>` if provided. */
  resetUrl?: string;
}

/** POST /auth/reset-password (public, no JWT) */
export interface ResetPasswordFromTokenRequest {
  token: string;
  newPassword: string;
}

export interface UserShort {
  id: string;
  firstname?: string | null;
  lastname?: string | null;
  accountType?: string | null;
  publicType?: string | null;
  nbLoans?: number | null;
  nbLateLoans?: number | null;
  loans?: Loan[];
  /** Subscription / membership expiry (ISO). Null = unlimited. */
  expiryAt?: string | null;
  createdAt?: string | null;
}

export interface LoginRequest {
  username: string;
  password: string;
  deviceId?: string;
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
  user: {
    id: string;
    username: string;
    login: string;
    firstname?: string;
    lastname?: string;
    accountType: string;
    language: string;
  };
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
  lastname?: string | null;
  firstname?: string | null;
  bio?: string | null;
  notes?: string | null;
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

/** Simplified physical copy (Item) as returned inside BiblioShort */
export interface ItemShort {
  id: string;
  barcode?: string | null;
  callNumber?: string | null;
  borrowable?: boolean | null;
  sourceId?: string | null;
  sourceName?: string | null;
  borrowed?: boolean;
  /** Item-level exception state: 0 available, 1 lost, 2 damaged, 3 claimed-returned. */
  circulationStatus?: number | null;
}

/** Short bibliographic record as returned in list endpoints */
export interface BiblioShort {
  id: string;
  mediaType?: MediaType | string | null;
  isbn?: string | null;
  title?: string | null;
  date?: string | null;
  status?: number | null;
  isLocal?: number | null;
  isValid?: number | null;
  archivedAt?: string | null;
  /** Simplified list of physical items (replaces nb_items / nb_available) */
  items?: ItemShort[];
  author?: Author | null;
  sourceName?: string | null;
}

/** Full physical copy (exemplaire) */
export interface Item {
  id: string;
  biblioId?: string | null;
  sourceId?: string | null;
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  circulationStatus?: number | null;
  notes?: string | null;
  price?: string | null;
  createdAt?: string | null;
  updatedAt?: string | null;
  archivedAt?: string | null;
  sourceName?: string | null;
  borrowed?: boolean;
  /** Active loan id when borrowed (staff circulation lookup). */
  loanId?: string | null;
}

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
  genre?: number | null;
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

/** Physical item data when creating a Biblio in one request (POST /biblios with items) */
export interface CreateBiblioItemInput {
  barcode?: string | null;
  callNumber?: string | null;
  sourceId: string;
}

/** Payload for POST /biblios/{id}/items */
export interface CreateItem {
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  notes?: string | null;
  price?: string | null;
  /** Write-only: treat a missing price as an explicit deferral when enabling circulation. */
  priceDeferred?: boolean | null;
  sourceId?: string | null;
  sourceName?: string | null;
}

/** Payload for PUT /items/{id} */
export interface UpdateItem {
  barcode?: string | null;
  callNumber?: string | null;
  volumeDesignation?: string | null;
  place?: number | null;
  borrowable?: boolean | null;
  notes?: string | null;
  price?: string | null;
  /** Write-only: treat a missing price as an explicit deferral when enabling circulation. */
  priceDeferred?: boolean | null;
  sourceId?: string | null;
  sourceName?: string | null;
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

// Loan types
export interface Loan {
  id: string;
  startDate: string;
  expiryAt: string;
  /** Present when the loan has been returned */
  returnedAt?: string | null;
  renewalDate?: string | null;
  nbRenews: number;
  /** Bibliographic record associated with the loan */
  biblio: BiblioShort;
  user?: UserShort;
  itemIdentification?: string | null;
  isOverdue: boolean;
}

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

// Aggregate user stats from /stats/users?mode=aggregate
export interface UserAggregateStats {
  newUsersTotal: number;
  activeBorrowersTotal: number;
  usersTotal: number;
  newUsersByPublicType?: StatEntry[];
  activeBorrowersByPublicType?: StatEntry[];
  usersByPublicType?: StatEntry[];
  usersBySex?: StatEntry[];
  newUsersBySex?: StatEntry[];
  activeBorrowersBySex?: StatEntry[];
  groupsTotal?: number;
}

// Time-based stats for charts
export interface LoanTimeStats {
  date: string;
  loans: number;
  returns: number;
}

export interface UserLoanStats {
  userId: string;
  firstname: string;
  lastname: string;
  totalLoans: number;
  activeLoans: number;
  overdueLoans: number;
}

// Catalog stats from /stats/catalog
export interface CatalogStatsBreakdown {
  label?: string;
  sourceId?: string;
  sourceName?: string;
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
}

export interface MarcBatchImportReport {
  batchId: string;
  /** IDs of successfully imported records */
  imported: string[];
  failed: MarcBatchImportError[];
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
  | 'duplicate_isbn_needs_confirmation'
  | 'duplicate_barcode_needs_confirmation'
  | 'z3950_error'
  | 'database_error'
  | 'internal_error';

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
  /** Override of global unpaid-fine threshold. null inherits circulation policy. */
  unpaidFineThreshold?: string | null;
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
  /** Omit or null to inherit the global circulation policy. */
  unpaidFineThreshold?: string | null;
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
   * When set, replaces the unpaid-fine threshold override.
   * Omit to keep the current value (API does not treat null as “inherit”).
   */
  unpaidFineThreshold?: string | null;
  /**
   * When set, replaces the max-active-holds override.
   * Omit to keep the current value (API does not treat null as “inherit”).
   */
  maxActiveHolds?: number | null;
}

// Source type
export interface Source {
  id: string;
  key: string | null;
  name: string | null;
  isArchive?: number | null;
  archivedAt?: string | null;
  default?: boolean;
}

/** Library role with permission flags — GET /account-types (camelCase). */
export interface AccountTypeDefinition {
  code: string;
  name: string;
  itemsRights: string | null;
  usersRights: string | null;
  loansRights: string | null;
  itemsArchiveRights: string | null;
  borrowsRights: string | null;
  settingsRights: string | null;
  eventsRights: string | null;
  /** Present after the acquisitions domain lands (`n`/`r`/`w`). */
  acquisitionsRights?: string | null;
}

export type AccountTypeRightLevel = 'n' | 'r' | 'w';

/** PUT /account-types/:code — partial body; each right must be n, r, w, or null when clearing. */
export interface UpdateAccountTypeRequest {
  name?: string;
  itemsRights?: AccountTypeRightLevel | null;
  usersRights?: AccountTypeRightLevel | null;
  loansRights?: AccountTypeRightLevel | null;
  itemsArchiveRights?: AccountTypeRightLevel | null;
  borrowsRights?: AccountTypeRightLevel | null;
  settingsRights?: AccountTypeRightLevel | null;
  eventsRights?: AccountTypeRightLevel | null;
  acquisitionsRights?: AccountTypeRightLevel | null;
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

/** Profile fields that may carry domain rights (flat or nested `rights`). */
export type RightsBearer = Pick<
  User,
  | 'holdsRights'
  | 'itemsRights'
  | 'usersRights'
  | 'loansRights'
  | 'settingsRights'
  | 'eventsRights'
  | 'acquisitionsRights'
  | 'rights'
>;

const PROFILE_FLAT_KEYS: Record<RightsDomain, keyof RightsBearer> = {
  items: 'itemsRights',
  users: 'usersRights',
  loans: 'loansRights',
  holds: 'holdsRights',
  settings: 'settingsRights',
  events: 'eventsRights',
  acquisitions: 'acquisitionsRights',
};

function pickFromAuthMeRights(bag: AuthMeRights | null | undefined, domain: RightsDomain): unknown {
  if (!bag) return undefined;
  switch (domain) {
    case 'items':
      return bag.itemsRights;
    case 'users':
      return bag.usersRights;
    case 'loans':
      return bag.loansRights;
    case 'holds':
      return bag.holdsRights ?? bag.borrowsRights;
    case 'settings':
      return bag.settingsRights;
    case 'events':
      return bag.eventsRights;
    case 'acquisitions':
      return bag.acquisitionsRights;
  }
}

/** Domain right from profile only (`none` | `own` | `read` | `write`). */
export function resolveDomainRightFromProfile(
  user: RightsBearer | null | undefined,
  domain: RightsDomain,
): RightsLevel | null {
  if (!user) return null;
  const nested = user.rights && typeof user.rights === 'object' ? user.rights : null;
  const flatKey = PROFILE_FLAT_KEYS[domain];
  const flat = user[flatKey];
  return normalizeRightsLevel(pickFromAuthMeRights(nested, domain) ?? flat);
}

/**
 * Effective domain permission: JWT `rights.*` first (available right after login),
 * then GET /auth/me fields when present.
 */
export function resolveDomainRight(
  user: RightsBearer | null | undefined,
  domain: RightsDomain,
  authToken?: string | null,
): RightsLevel | null {
  const fromJwt = authToken ? readDomainRightFromJwt(authToken, domain) : null;
  if (fromJwt) return fromJwt;
  return resolveDomainRightFromProfile(user, domain);
}

export function hasDomainRight(
  user: RightsBearer | null | undefined,
  domain: RightsDomain,
  required: RightsLevel,
  authToken?: string | null,
): boolean {
  return hasMinRightsLevel(resolveDomainRight(user, domain, authToken), required);
}

/** Catalog mutate (create/edit biblio & items, Z39.50 import write). */
export const canManageItems = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'items', 'write', authToken);

/** Staff catalog list/detail when JWT grants catalog read. */
export const canReadItems = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'items', 'read', authToken);

export const canManageUsers = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'users', 'write', authToken);

export const canReadUsers = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'users', 'read', authToken);

/** Circulation desk ops (checkout / return / renew) — matches `require_write_loans`. */
export const canManageLoans = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'loans', 'write', authToken);

export const canReadLoans = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'loans', 'read', authToken);

/** Normalized holds level from profile only (`none` | `own` | `read` | `write`). */
export function resolveHoldsRightsFromProfile(
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
): string | null {
  return resolveDomainRightFromProfile(user, 'holds');
}

/**
 * Effective holds permission: JWT `rights.holdsRights` first (available right after login),
 * then GET /auth/me fields when present.
 */
export function resolveHoldsRights(
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): string | null {
  return resolveDomainRight(user, 'holds', authToken);
}

/** Personal holds UI (Mes réservations, réserver un exemplaire) for `own`, `read`, or `write`. */
export const canPatronSelfServiceHolds = (
  user: Pick<User, 'holdsRights' | 'rights'> | null | undefined,
  authToken?: string | null,
): boolean => {
  const r = resolveHoldsRights(user, authToken);
  return r === 'own' || r === 'read' || r === 'write';
};

/** Staff holds desk list/queues — `holdsRights` ≥ read (`own` alone is insufficient). */
export const canViewStaffHolds = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'holds', 'read', authToken);

/** Create/cancel holds for others — `holdsRights` ≥ write. */
export const canManageStaffHolds = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'holds', 'write', authToken);

/** Stats overview — catalog read is the common gate for `/stats` entry. */
export const canViewStats = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'items', 'read', authToken);

/** Full settings mutate (non-admin tabs still need admin where API uses require_admin). */
export const canManageSettings = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'settings', 'write', authToken);

export const canManageEvents = (
  user: RightsBearer | null | undefined,
  authToken?: string | null,
): boolean => hasDomainRight(user, 'events', 'write', authToken);

/** Normalized acquisitions level: `n` | `r` | `w`. */
export function normalizeAcquisitionsRightsLevel(raw: unknown): AccountTypeRightLevel | null {
  const level = normalizeRightsLevel(raw);
  if (level === 'none') return 'n';
  if (level === 'read') return 'r';
  if (level === 'write') return 'w';
  return null;
}

export function resolveAcquisitionsRightsFromProfile(
  user: Pick<User, 'acquisitionsRights' | 'rights'> | null | undefined,
): AccountTypeRightLevel | null {
  return normalizeAcquisitionsRightsLevel(resolveDomainRightFromProfile(user, 'acquisitions'));
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
export type AdminConfigSectionKey = 'email' | 'logging' | 'reminders' | 'audit' | 'holds';

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

// Overdue loans dashboard
export interface OverdueLoanInfo {
  loanId: string;
  userId: string;
  firstname?: string;
  lastname?: string;
  userEmail?: string;
  biblioId?: string;
  title?: string;
  authors?: string;
  itemBarcode?: string;
  loanDate: string;
  expiryAt: string | null;
  lastReminderSentAt: string | null;
  reminderCount: number;
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

export type HoldStatus = 'pending' | 'ready' | 'fulfilled' | 'cancelled' | 'expired';

/**
 * Hold detail rows (GET list endpoints). `biblio.items` is empty for an
 * unassigned title-level hold; otherwise exactly one {@link ItemShort}.
 */
export interface Hold {
  id: string;
  userId: string;
  biblioId: string;
  /** Null while a title-level hold is waiting for a copy. */
  itemId?: string | null;
  pickupSiteId?: string | null;
  createdAt: string;
  notifiedAt: string | null;
  expiresAt: string | null;
  status: HoldStatus;
  position: number;
  notes: string | null;
  /** Populated on GET /holds, GET /items/:id/holds, GET /biblios/:id/holds, GET /users/:id/holds */
  biblio?: BiblioShort | null;
  user?: UserShort | null;
}

export interface CreateHold {
  userId: string;
  /** Pinned copy. Omit for a title-level hold. */
  itemId?: string;
  /** Always sent. Required when `itemId` is omitted. */
  biblioId?: string;
  pickupSiteId?: string | null;
  notes?: string | null;
  force?: boolean;
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

export interface ItemTransit {
  id: string;
  itemId: string;
  holdId?: string | null;
  fromSourceId: string;
  toSourceId: string;
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
}

/** POST /holds/{id}/transits — allocate a copy toward the hold’s pickup site. */
export interface CreateTransit {
  itemId?: string | null;
  fromSourceId?: string | null;
  /** Must match hold.pickupSiteId when that field is already set. */
  toSourceId?: string | null;
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
  toSourceId?: string;
  page?: number;
  perPage?: number;
}

// ──────────────────────────────────────────────────────────────────
// Fines / Penalties
// ──────────────────────────────────────────────────────────────────

export type FineStatus = 'pending' | 'partial' | 'paid' | 'waived';

export type FineChargeType = 'overdue' | 'replacement' | 'damage';

export interface Fine {
  id: string;
  loanId?: string;
  userId?: string;
  amount: string;
  paidAmount?: string;
  status: FineStatus;
  createdAt?: string;
  paidAt?: string | null;
  notes?: string | null;
  chargeType?: FineChargeType;
}

export interface FinesResponse {
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

/** GET/PUT /fines/policy — global unpaid-fine threshold (camelCase). */
export interface CirculationFinePolicy {
  unpaidFineThreshold: string;
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
  /** Catalog source filter (`items.source_id`); null = all sources */
  scopeSourceId?: string | null;
  scopeSourceName?: string | null;
  consolidatedAt?: string | null;
  consolidatedBy?: string | null;
}

export interface CreateInventorySession {
  name: string;
  locationFilter?: string | null;
  notes?: string | null;
  scopePlace?: number | null;
  scopeSourceId?: string | null;
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
  sourceId?: string | null;
  sourceName?: string | null;
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
  sourceId?: string | null;
  sourceName?: string | null;
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

export interface BatchReturnResult {
  barcode: string;
  success: boolean;
  loan?: Loan | null;
  error?: string | null;
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
  error?: string | null;
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
  sourceId?: string | null;
  sourceName?: string | null;
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
