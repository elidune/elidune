import type { PublicType } from '@/types';

/** Circulation audiences seeded in English. `name` is stable; `label` is the seed caption. */
const SEEDED_PATRON_TYPES: Record<string, { englishLabel: string; key: string }> = {
  child: { englishLabel: 'Child', key: 'codes.patronType.child' },
  adult: { englishLabel: 'Adult', key: 'codes.patronType.adult' },
  school: { englishLabel: 'School', key: 'codes.patronType.school' },
  staff: { englishLabel: 'Staff', key: 'codes.patronType.staff' },
  senior: { englishLabel: 'Senior', key: 'codes.patronType.senior' },
};

type PublicTypeCaption = Pick<PublicType, 'name' | 'label'>;
type PublicTypeRow = PublicTypeCaption & Pick<PublicType, 'id'>;

/**
 * Label for a circulation audience on patron and event screens.
 * Seeded names (child, adult, school, staff, senior) go through i18n
 * while their caption is still the English seed. A library-defined caption is kept.
 */
export function localizedPublicTypeLabel(
  t: (key: string) => string,
  type: PublicTypeCaption | null | undefined,
): string {
  if (!type) return '';
  const name = type.name?.trim() ?? '';
  const label = type.label?.trim() ?? '';
  const seeded = SEEDED_PATRON_TYPES[name.toLowerCase()];
  if (!seeded) return label || name;

  const folded = label.toLowerCase();
  const isSeedCaption =
    folded === '' ||
    folded === seeded.englishLabel.toLowerCase() ||
    folded === name.toLowerCase();
  if (!isSeedCaption) return label;

  const translated = t(seeded.key);
  return translated && translated !== seeded.key ? translated : label || name;
}

/** Translated caption for a patron's public-type id, or null when unset or unknown. */
export function localizedPublicTypeById(
  t: (key: string) => string,
  publicTypes: readonly PublicTypeRow[],
  id: string | number | null | undefined,
): string | null {
  if (id == null || String(id).trim() === '') return null;
  const match = publicTypes.find((row) => row.id === String(id));
  if (!match) return null;
  const label = localizedPublicTypeLabel(t, match);
  return label === '' ? null : label;
}
