export type MediaKind = "image" | "video" | "audio" | "pdf";

export interface ThumbnailRequest {
  kind: MediaKind;
  width: number;
  height: number;
}

export class ThumbnailError extends Error {
  constructor(
    readonly exitCode: 2 | 3 | 4,
    message: string,
  ) {
    super(message);
  }
}

export function parseThumbnailRequest(
  args: readonly string[],
): ThumbnailRequest {
  const [kind, rawWidth, rawHeight] = args;
  if (
    args.length !== 3 ||
    (kind !== "image" && kind !== "video" && kind !== "audio" && kind !== "pdf")
  )
    throw new ThumbnailError(2, "Invalid thumbnail request");
  const width = Number(rawWidth);
  const height = Number(rawHeight);
  if (
    ![width, height].every(
      (value) => Number.isInteger(value) && value >= 16 && value <= 512,
    )
  ) {
    throw new ThumbnailError(2, "Invalid thumbnail dimensions");
  }
  return { kind, width, height };
}
