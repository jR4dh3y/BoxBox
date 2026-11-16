// Downloads in the preview are real: text is saved as written, and a sample
// photo is rasterised so a file named .jpg holds a JPEG.

import type { FileEntry } from './fs';

function save(blob: Blob, name: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = name;
  link.click();
  URL.revokeObjectURL(url);
}

export function downloadFile(file: FileEntry): void {
  if (file.kind === 'text') {
    save(new Blob([file.content], { type: 'text/plain' }), file.name);
    return;
  }
  const image = new Image();
  image.addEventListener('load', () => {
    const canvas = document.createElement('canvas');
    canvas.width = 1600;
    canvas.height = 1000;
    canvas.getContext('2d')?.drawImage(image, 0, 0, canvas.width, canvas.height);
    canvas.toBlob((blob) => {
      if (blob) save(blob, file.name);
    }, 'image/jpeg', 0.9);
  });
  image.src = file.src;
}
