export type MarcFileEncoding =
  | 'utf8'
  | 'marc8'
  | 'iso5426'
  | 'iso8859-2'
  | 'iso8859-3'
  | 'iso8859-5';

export const DEFAULT_MARC_FILE_ENCODING: MarcFileEncoding = 'utf8';

export const MARC_FILE_ENCODINGS: MarcFileEncoding[] = [
  'utf8',
  'marc8',
  'iso5426',
  'iso8859-2',
  'iso8859-3',
  'iso8859-5',
];

export function isMarcFileEncoding(value: string): value is MarcFileEncoding {
  return (MARC_FILE_ENCODINGS as string[]).includes(value);
}

/** i18n key under `importMarc.encoding.*` */
export function marcFileEncodingLabelKey(encoding: MarcFileEncoding): string {
  return `importMarc.encoding.${encoding.replace(/-/g, '_')}`;
}
