// A pretend upload: bytes are counted, never sent. Files land in the tree one
// at a time, each under its own relative path.

import { entryBytes, type FileEntry } from './fs';

export type UploadItem = { relPath: string; file: FileEntry };
export type Upload = { label: string; target: string; items: UploadItem[]; sent: number };

export type UploadProgress = {
  /** The file being sent, or the last one once the upload is done. */
  current: UploadItem | undefined;
  currentSent: number;
  currentBytes: number;
  completed: number;
  percent: number;
  done: boolean;
};

export function uploadBytes(upload: Upload): number {
  return upload.items.reduce((sum, item) => sum + entryBytes(item.file), 0);
}

export function progressOf(upload: Upload): UploadProgress {
  const total = uploadBytes(upload);
  const done = upload.sent >= total;
  let before = 0;
  let completed = 0;
  let current = upload.items.at(-1);
  for (const item of upload.items) {
    const bytes = entryBytes(item.file);
    if (upload.sent < before + bytes) {
      current = item;
      break;
    }
    before += bytes;
    completed += 1;
  }
  const currentBytes = current ? entryBytes(current.file) : 0;
  return {
    current,
    currentSent: done ? currentBytes : upload.sent - before,
    currentBytes,
    completed,
    percent: total === 0 ? 100 : Math.min(100, Math.floor((upload.sent / total) * 100)),
    done
  };
}

/** Sends `bytes` more and reports the files that finished because of it. */
export function advance(upload: Upload, bytes: number): { upload: Upload; finished: UploadItem[] } {
  const next = { ...upload, sent: Math.min(upload.sent + bytes, uploadBytes(upload)) };
  const finished = upload.items.slice(progressOf(upload).completed, progressOf(next).completed);
  return { upload: next, finished };
}
