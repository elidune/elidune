import type { PublicType, UserShort } from '@/types';

export const CHILD_PUBLIC_TYPE_NAME = 'child';
export const SCHOOL_PUBLIC_TYPE_NAME = 'school';

const GUARDIAN_SERVER_MESSAGES: Record<string, string> = {
  'guardianid is required for child patrons': 'users.guardianErrors.required',
  'guardian must be a major patron (not child or school)': 'users.guardianErrors.notMajor',
  'guardianid cannot be the same patron': 'users.guardianErrors.samePatron',
  'guardianid does not refer to an existing patron': 'users.guardianErrors.notFound',
  'guardian must be an active patron': 'users.guardianErrors.inactive',
};

export function findPublicTypeById(
  publicTypes: PublicType[],
  id: string | number | null | undefined
): PublicType | undefined {
  if (id == null || id === '') return undefined;
  const key = String(id);
  return publicTypes.find((pt) => pt.id === key);
}

/** Guardian is required only for seeded public type `child`. */
export function publicTypeRequiresGuardian(
  publicTypes: PublicType[],
  publicTypeId: string | number | null | undefined
): boolean {
  return findPublicTypeById(publicTypes, publicTypeId)?.name === CHILD_PUBLIC_TYPE_NAME;
}

/** A guardian must be a major patron: not `child` and not `school`. Unknown type: allow (server decides). */
export function publicTypeCanBeLegalGuardian(pt: PublicType | undefined): boolean {
  if (!pt) return true;
  return pt.name !== CHILD_PUBLIC_TYPE_NAME && pt.name !== SCHOOL_PUBLIC_TYPE_NAME;
}

export function isEligibleGuardianCandidate(
  user: UserShort,
  publicTypes: PublicType[],
  excludeUserId?: string
): boolean {
  if (excludeUserId && user.id === excludeUserId) return false;
  return publicTypeCanBeLegalGuardian(findPublicTypeById(publicTypes, user.publicType));
}

export function guardianValidationI18nKey(message: string | null | undefined): string | null {
  if (!message) return null;
  return GUARDIAN_SERVER_MESSAGES[message.trim().toLowerCase()] ?? null;
}
