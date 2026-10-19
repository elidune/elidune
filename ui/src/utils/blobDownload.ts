/** Delay before revoking blob URLs so the browser can start the download (Firefox). */
const REVOKE_BLOB_URL_MS = 60_000;

/**
 * Programmatic file download from a Blob. Revokes the object URL after a delay.
 */
export function triggerBlobDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = 'noopener';
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), REVOKE_BLOB_URL_MS);
}

function suggestedDumpFilename(): string {
  const stamp = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z').replace(/[-:]/g, '');
  return `elidune-db-dump-${stamp}.sql`;
}

/**
 * When supported, opens the native save dialog during the user gesture (before a long fetch).
 * Returns a writable target or null when the user cancels or the API is unavailable.
 */
type SqlSavePickerOptions = {
  suggestedName?: string;
  types?: Array<{ description?: string; accept: Record<string, string[]> }>;
};

type SaveFilePickerWindow = Window & {
  showSaveFilePicker?: (options: SqlSavePickerOptions) => Promise<FileSystemFileHandle>;
};

export async function pickSqlDumpSaveTarget(): Promise<FileSystemFileHandle | null> {
  const picker = (window as SaveFilePickerWindow).showSaveFilePicker;
  if (typeof picker !== 'function') {
    return null;
  }
  try {
    return await picker({
      suggestedName: suggestedDumpFilename(),
      types: [
        {
          description: 'SQL',
          accept: { 'application/sql': ['.sql'], 'text/plain': ['.sql'] },
        },
      ],
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === 'AbortError') {
      return null;
    }
    return null;
  }
}

export async function writeBlobToFileHandle(handle: FileSystemFileHandle, blob: Blob): Promise<void> {
  const writable = await handle.createWritable();
  await writable.write(blob);
  await writable.close();
}

export function normalizeSqlBlob(blob: Blob): Blob {
  if (blob.type && blob.type !== 'application/octet-stream') {
    return blob;
  }
  return new Blob([blob], { type: 'application/sql' });
}
