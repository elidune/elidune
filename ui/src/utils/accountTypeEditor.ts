import type {
  AccountTypeDefinition,
  AccountTypeHoldsRightLevel,
  AccountTypeRightLevel,
  CreateAccountTypeRequest,
  UpdateAccountTypeRequest,
} from '@/types';

/** Permission flags on `AccountTypeDefinition` (`*Rights`), including new server columns. */
export type AccountTypeRightField = Extract<keyof AccountTypeDefinition, `${string}Rights`>;

/**
 * Display and payload order. Must list every `AccountTypeRightField`.
 * Adding a `*Rights` field on `AccountTypeDefinition` fails the check below
 * until this array includes it.
 */
export const ACCOUNT_TYPE_RIGHT_FIELDS = [
  'itemsRights',
  'usersRights',
  'loansRights',
  'circulationOverrideRights',
  'itemsArchiveRights',
  'holdsRights',
  'settingsRights',
  'eventsRights',
  'acquisitionsRights',
  'weedingRights',
  'weedingCancelRights',
] as const satisfies readonly AccountTypeRightField[];

type ListedAccountTypeRight = (typeof ACCOUNT_TYPE_RIGHT_FIELDS)[number];
type MissingAccountTypeRight = Exclude<AccountTypeRightField, ListedAccountTypeRight>;
type AccountTypeRightsListComplete = [MissingAccountTypeRight] extends [never] ? true : never;
const accountTypeRightsListComplete: AccountTypeRightsListComplete = true;
void accountTypeRightsListComplete;

type OwnableAccountTypeRight = 'holdsRights' | 'usersRights' | 'loansRights';

export function accountTypeRightAllowsOwn(
  field: AccountTypeRightField,
): field is OwnableAccountTypeRight {
  return field === 'holdsRights' || field === 'usersRights' || field === 'loansRights';
}

export type AccountTypeFieldError = 'required' | 'invalid' | 'tooLong';

export function normAccountTypeRight(value: string | null | undefined): string {
  const letter = (value ?? '').trim().toLowerCase();
  return letter === 'n' || letter === 'o' || letter === 'r' || letter === 'w' ? letter : '';
}

export function isBuiltInAccountType(row: Pick<AccountTypeDefinition, 'isSystem'>): boolean {
  return row.isSystem === true;
}

export function accountTypeCodeError(raw: string): AccountTypeFieldError | null {
  const code = raw.trim().toLowerCase();
  if (!code) return 'required';
  if (!/^[a-z][a-z0-9_]{0,49}$/.test(code)) return 'invalid';
  return null;
}

export function accountTypeNameError(raw: string): AccountTypeFieldError | null {
  const name = raw.trim();
  if (!name) return 'required';
  if (name.length > 100) return 'tooLong';
  return null;
}

export function blankAccountTypeRights(): Record<AccountTypeRightField, string> {
  return Object.fromEntries(ACCOUNT_TYPE_RIGHT_FIELDS.map((field) => [field, 'n'])) as Record<
    AccountTypeRightField,
    string
  >;
}

function standardLevel(value: string): AccountTypeRightLevel {
  return value === 'r' || value === 'w' ? value : 'n';
}

function ownLevel(value: string): AccountTypeHoldsRightLevel {
  return value === 'o' || value === 'r' || value === 'w' ? value : 'n';
}

export function buildCreateAccountTypeRequest(
  codeRaw: string,
  nameRaw: string,
  rights: Record<AccountTypeRightField, string>,
): CreateAccountTypeRequest | null {
  if (accountTypeCodeError(codeRaw) || accountTypeNameError(nameRaw)) return null;
  const code = codeRaw.trim().toLowerCase();
  const name = nameRaw.trim();
  const body: CreateAccountTypeRequest = { code, name };
  for (const field of ACCOUNT_TYPE_RIGHT_FIELDS) {
    const raw = normAccountTypeRight(rights[field]);
    if (accountTypeRightAllowsOwn(field)) {
      body[field] = ownLevel(raw);
    } else {
      body[field] = standardLevel(raw);
    }
  }
  return body;
}

export function buildAccountTypeUpdate(
  orig: AccountTypeDefinition,
  next: AccountTypeDefinition,
): UpdateAccountTypeRequest | null {
  const body: UpdateAccountTypeRequest = {};
  if ((next.name ?? '').trim() !== (orig.name ?? '').trim()) {
    body.name = (next.name ?? '').trim();
  }
  for (const field of ACCOUNT_TYPE_RIGHT_FIELDS) {
    const previous = normAccountTypeRight(orig[field]);
    const current = normAccountTypeRight(next[field]);
    if (previous === current) continue;
    if (accountTypeRightAllowsOwn(field)) {
      body[field] = current === '' ? null : ownLevel(current);
    } else if (current === '' || current === 'n' || current === 'r' || current === 'w') {
      body[field] = current === '' ? null : standardLevel(current);
    }
  }
  if (Object.keys(body).length === 0) return null;
  return body;
}

export function accountTypeUpdateChangesRights(body: UpdateAccountTypeRequest): boolean {
  return ACCOUNT_TYPE_RIGHT_FIELDS.some((field) => field in body);
}
