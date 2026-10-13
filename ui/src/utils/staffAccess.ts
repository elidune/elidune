import {
  canCreateHold,
  canManageAcquisitions,
  canManageItems,
  canManageWeeding,
  canReadHoldsStaff,
  canReadItems,
  canReadLoans,
  canReadSettings,
  canReadUsers,
  canViewAcquisitions,
  canViewStats,
  canViewWeeding,
  canWriteEvents,
  canWriteSettings,
  seesInternalCatalog,
  type User,
} from '@/types';

type AccessUser = Pick<
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

export interface StaffAccess {
  /** Staff catalog JSON (#217). Readers with only `itemsRights` read stay public. */
  internalCatalog: boolean;
  itemsRead: boolean;
  itemsWrite: boolean;
  loansRead: boolean;
  holdsStaff: boolean;
  holdsCreate: boolean;
  usersRead: boolean;
  settingsRead: boolean;
  settingsWrite: boolean;
  eventsWrite: boolean;
  weeding: boolean;
  acquisitions: boolean;
  /** Home statistics cards call `GET /stats` (`itemsRights` read) and only for the staff view. */
  homeStats: boolean;
  stats: boolean;
  /**
   * Library identity screen. `settingsRights` read is not enough on its own:
   * seeded readers also have settings read, and they stay on the public shell.
   * Settings write always opens the full settings screen.
   */
  librarySettings: boolean;
  fullSettings: boolean;
  backgroundTasks: boolean;
}

/**
 * Where to send someone who opened a reader record without users read.
 * Loans read or write stays on the circulation desk. Everyone else goes home.
 */
export function readerRecordFallback(access: Pick<StaffAccess, 'loansRead'>): string {
  return access.loansRead ? '/loans' : '/home';
}

/** Menu and route flags. Account type is not consulted. */
export function staffAccess(user: AccessUser, authToken?: string | null): StaffAccess {
  const internalCatalog = seesInternalCatalog(user, authToken);
  const settingsRead = canReadSettings(user, authToken);
  const settingsWrite = canWriteSettings(user, authToken);
  const eventsWrite = canWriteEvents(user, authToken);
  return {
    internalCatalog,
    itemsRead: canReadItems(user, authToken),
    itemsWrite: canManageItems(user, authToken),
    loansRead: canReadLoans(user, authToken),
    holdsStaff: canReadHoldsStaff(user, authToken),
    holdsCreate: canCreateHold(user, authToken),
    usersRead: canReadUsers(user, authToken),
    settingsRead,
    settingsWrite,
    eventsWrite,
    weeding: canViewWeeding(user, authToken),
    acquisitions: canViewAcquisitions(user, authToken),
    homeStats: internalCatalog && canReadItems(user, authToken),
    stats: canViewStats(user, authToken),
    librarySettings: settingsRead && (settingsWrite || internalCatalog),
    fullSettings: settingsWrite,
    backgroundTasks:
      internalCatalog ||
      settingsWrite ||
      eventsWrite ||
      canManageWeeding(user, authToken) ||
      canManageAcquisitions(user, authToken),
  };
}
