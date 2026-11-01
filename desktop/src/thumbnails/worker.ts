import { fstatSync } from "node:fs";
import { join } from "node:path";
import { loadThumbnailLibrary } from "./native";
import { parseThumbnailRequest, ThumbnailError } from "./request";

export async function renderThumbnail(
  args: readonly string[],
  libraryPath: string,
): Promise<Uint8Array> {
  const request = parseThumbnailRequest(args);
  if (!fstatSync(3).isFile())
    throw new ThumbnailError(2, "Thumbnail source must be a regular file");
  const library = loadThumbnailLibrary(libraryPath);
  try {
    return library.fromDescriptor(
      3,
      request.kind,
      request.width,
      request.height,
    );
  } finally {
    library.close();
  }
}

if (import.meta.main) {
  try {
    const image = await renderThumbnail(
      process.argv.slice(2),
      join(import.meta.dir, "libboxbox_thumbnail.so"),
    );
    await Bun.write(Bun.stdout, image);
  } catch (error) {
    console.error(
      error instanceof Error ? error.message : "Thumbnail rendering failed",
    );
    process.exitCode = error instanceof ThumbnailError ? error.exitCode : 4;
  }
}
