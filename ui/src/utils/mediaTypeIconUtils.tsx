import type { ReactNode } from 'react';
import type { LucideIcon } from 'lucide-react';
import {
  AudioLines,
  BookImage,
  BookOpenText,
  CassetteTape,
  CircleHelp,
  Clapperboard,
  Disc3,
  DiscAlbum,
  FileText,
  HardDrive,
  Image,
  Layers,
  Library,
  Mic,
  Music,
  Newspaper,
} from 'lucide-react';
import type { MediaType } from '@/types';

const LEGACY_MEDIA_TYPE_MAP: Record<string, MediaType> = {
  u: 'unknown',
  b: 'printedText',
  bc: 'comics',
  p: 'periodic',
  v: 'video',
  vt: 'videoTape',
  vd: 'videoDvd',
  a: 'audio',
  am: 'audioMusic',
  amt: 'audioMusicTape',
  amc: 'audioMusicCd',
  an: 'audioNonMusic',
  ant: 'audioNonMusicTape',
  anc: 'audioNonMusicCd',
  c: 'cdRom',
  i: 'images',
  m: 'multimedia',
};

const KNOWN_MEDIA_TYPES = new Set<string>([
  'unknown',
  'printedText',
  'multimedia',
  'comics',
  'periodic',
  'video',
  'videoTape',
  'videoDvd',
  'audio',
  'audioMusic',
  'audioMusicTape',
  'audioMusicCd',
  'audioNonMusic',
  'audioNonMusicTape',
  'audioNonMusicCd',
  'cdRom',
  'images',
  'all',
]);

/** Canonical media type for icons (legacy slugs + unknown strings → fallback). */
export function normalizeMediaType(
  mediaType: MediaType | string | null | undefined,
): MediaType | 'unknown' {
  if (mediaType == null || mediaType === '') return 'unknown';
  const raw = String(mediaType).trim();
  const legacy = LEGACY_MEDIA_TYPE_MAP[raw];
  if (legacy) return legacy;
  if (KNOWN_MEDIA_TYPES.has(raw)) {
    return raw === 'all' ? 'all' : (raw as MediaType);
  }
  return 'unknown';
}

const MEDIA_TYPE_ICON: Record<MediaType | 'unknown', LucideIcon> = {
  unknown: CircleHelp,
  all: Library,
  printedText: BookOpenText,
  comics: BookImage,
  periodic: Newspaper,
  video: Clapperboard,
  videoTape: CassetteTape,
  videoDvd: Disc3,
  audio: AudioLines,
  audioMusic: Music,
  audioMusicTape: CassetteTape,
  audioMusicCd: DiscAlbum,
  audioNonMusic: Mic,
  audioNonMusicTape: CassetteTape,
  audioNonMusicCd: Disc3,
  cdRom: HardDrive,
  images: Image,
  multimedia: Layers,
};

const DEFAULT_GLYPH_CLASS = 'text-indigo-600 dark:text-indigo-400';

/** Background for the icon badge — same indigo tile as BiblioDetailPage header. */
export function mediaTypeIconBadgeBgClass(_mediaType?: MediaType | string | null): string {
  return 'bg-indigo-100 dark:bg-indigo-900/50';
}

/** Icon only (no wrapper). Pass full Tailwind size classes, e.g. `h-5 w-5`. */
export function renderMediaTypeIcon(
  mediaType: MediaType | string | null | undefined,
  iconClassName: string,
  glyphClassName: string = DEFAULT_GLYPH_CLASS,
): ReactNode {
  const normalized = normalizeMediaType(mediaType);
  const Icon = MEDIA_TYPE_ICON[normalized] ?? FileText;
  return <Icon className={[iconClassName, glyphClassName].filter(Boolean).join(' ')} aria-hidden />;
}
