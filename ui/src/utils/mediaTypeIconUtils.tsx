import type { ReactNode } from 'react';
import { BookOpen, Disc, FileText, Image, Music, Newspaper, Video } from 'lucide-react';
import type { MediaType } from '@/types';

/** Background for the icon badge — matches catalog list (BibliosPage). */
export function mediaTypeIconBadgeBgClass(mediaType?: MediaType | string | null): string {
  switch (mediaType) {
    case 'printedText':
    case 'comics':
      return 'bg-amber-50 dark:bg-amber-900/30';
    case 'periodic':
      return 'bg-orange-50 dark:bg-orange-900/30';
    case 'video':
    case 'videoTape':
    case 'videoDvd':
      return 'bg-red-50 dark:bg-red-900/30';
    case 'audio':
    case 'audioMusic':
    case 'audioMusicTape':
    case 'audioMusicCd':
    case 'audioNonMusic':
    case 'audioNonMusicTape':
    case 'audioNonMusicCd':
      return 'bg-blue-50 dark:bg-blue-900/30';
    case 'cdRom':
      return 'bg-purple-50 dark:bg-purple-900/30';
    case 'images':
      return 'bg-green-50 dark:bg-green-900/30';
    case 'multimedia':
      return 'bg-indigo-50 dark:bg-indigo-900/30';
    default:
      return 'bg-gray-50 dark:bg-gray-900/30';
  }
}

/** Icon only (no wrapper). Pass full Tailwind size classes, e.g. `h-5 w-5`. */
export function renderMediaTypeIcon(
  mediaType: MediaType | string | null | undefined,
  iconClassName: string,
): ReactNode {
  const c = iconClassName;
  const amber = 'text-amber-600 dark:text-amber-400';
  switch (mediaType) {
    case 'printedText':
    case 'comics':
      return <BookOpen className={`${c} ${amber}`} />;
    case 'periodic':
      return <Newspaper className={`${c} ${amber}`} />;
    case 'video':
    case 'videoTape':
    case 'videoDvd':
      return <Video className={`${c} text-red-600 dark:text-red-400`} />;
    case 'audio':
    case 'audioMusic':
    case 'audioMusicTape':
    case 'audioMusicCd':
    case 'audioNonMusic':
    case 'audioNonMusicTape':
    case 'audioNonMusicCd':
      return <Music className={`${c} text-blue-600 dark:text-blue-400`} />;
    case 'cdRom':
      return <Disc className={`${c} text-purple-600 dark:text-purple-400`} />;
    case 'images':
      return <Image className={`${c} text-green-600 dark:text-green-400`} />;
    case 'multimedia':
      return <FileText className={`${c} text-indigo-600 dark:text-indigo-400`} />;
    default:
      return <BookOpen className={`${c} text-gray-600 dark:text-gray-400`} />;
  }
}
