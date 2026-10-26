import { dlopen, FFIType } from "bun:ffi";
import { ThumbnailError } from "./request";
import type { MediaKind } from "./request";

const KIND: Record<MediaKind, number> = {
  image: 0,
  video: 1,
  audio: 2,
  pdf: 3,
};

export function loadThumbnailLibrary(path: string) {
  function openLibrary() {
    return dlopen(path, {
      boxbox_thumbnail_abi: { args: [], returns: FFIType.u32 },
      boxbox_thumbnail_fd: {
        args: [
          FFIType.u32,
          FFIType.u32,
          FFIType.u32,
          FFIType.u32,
          FFIType.ptr,
          FFIType.u64_fast,
        ],
        returns: FFIType.i32,
      },
    });
  }
  const library = (() => {
    try {
      return openLibrary();
    } catch {
      throw new ThumbnailError(4, "Native thumbnail library unavailable");
    }
  })();
  if (library.symbols.boxbox_thumbnail_abi() !== 1) {
    library.close();
    throw new ThumbnailError(4, "Unsupported thumbnail renderer ABI");
  }
  const output = new Uint8Array(1 << 20);
  function result(length: number): Uint8Array {
    if (length <= 0 || length > output.byteLength) {
      throw new ThumbnailError(
        length === -2 ? 3 : length === -4 ? 4 : 2,
        "Thumbnail source could not be rendered",
      );
    }
    return output.slice(0, length);
  }
  return {
    fromDescriptor(fd: number, kind: MediaKind, width: number, height: number) {
      return result(
        library.symbols.boxbox_thumbnail_fd(
          fd,
          KIND[kind],
          width,
          height,
          output,
          output.byteLength,
        ),
      );
    },
    close: () => library.close(),
  };
}
