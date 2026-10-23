import type { LucideIcon } from 'lucide-react';
import { BookOpen, Crown, Users } from 'lucide-react';

export type AccountTypeVisualKind =
  | 'admin'
  | 'librarian'
  | 'group'
  | 'guest'
  | 'reader'
  | 'default';

/** Maps API account-type codes (slug or legacy digit) to list avatar styling. */
export function accountTypeVisualKind(code: string | null | undefined): AccountTypeVisualKind {
  const normalized = String(code ?? '').trim().toLowerCase();
  switch (normalized) {
    case '4':
    case 'admin':
    case 'administrator':
      return 'admin';
    case '3':
    case 'librarian':
      return 'librarian';
    case '8':
    case 'group':
      return 'group';
    case '1':
    case 'guest':
      return 'guest';
    case '2':
    case 'reader':
      return 'reader';
    default:
      return 'default';
  }
}

type AccountTypeVisual = {
  circleClass: string;
  initialsClass: string;
  overlayIcon: LucideIcon | null;
  overlayIconClass: string;
};

const VISUALS: Record<AccountTypeVisualKind, AccountTypeVisual> = {
  admin: {
    circleClass: 'bg-amber-100 dark:bg-amber-900/45',
    initialsClass: 'text-amber-800 dark:text-amber-200',
    overlayIcon: Crown,
    overlayIconClass: 'text-amber-700 dark:text-amber-300',
  },
  librarian: {
    circleClass: 'bg-sky-100 dark:bg-sky-900/45',
    initialsClass: 'text-sky-800 dark:text-sky-200',
    overlayIcon: BookOpen,
    overlayIconClass: 'text-sky-700 dark:text-sky-300',
  },
  group: {
    circleClass: 'bg-violet-100 dark:bg-violet-900/45',
    initialsClass: 'text-violet-800 dark:text-violet-200',
    overlayIcon: Users,
    overlayIconClass: 'text-violet-700 dark:text-violet-300',
  },
  guest: {
    circleClass: 'bg-gray-100 dark:bg-gray-800',
    initialsClass: 'text-gray-600 dark:text-gray-300',
    overlayIcon: null,
    overlayIconClass: '',
  },
  reader: {
    circleClass: 'bg-indigo-100 dark:bg-indigo-900/50',
    initialsClass: 'text-indigo-600 dark:text-indigo-400',
    overlayIcon: null,
    overlayIconClass: '',
  },
  default: {
    circleClass: 'bg-slate-100 dark:bg-slate-800/80',
    initialsClass: 'text-slate-700 dark:text-slate-300',
    overlayIcon: null,
    overlayIconClass: '',
  },
};

export function accountTypeVisual(code: string | null | undefined): AccountTypeVisual {
  return VISUALS[accountTypeVisualKind(code)];
}
