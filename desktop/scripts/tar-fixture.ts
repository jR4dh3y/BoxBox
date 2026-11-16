// Writes minimal ustar archives for installer and runtime-archive tests.
export interface TarFixtureEntry {
  readonly name: string;
  readonly content?: string;
  readonly mode?: number;
  readonly type?: "file" | "directory" | "symlink";
  readonly linkName?: string;
}

export function createTar(entries: readonly TarFixtureEntry[]): Buffer {
  const blocks: Buffer[] = [];
  for (const entry of entries) {
    const type = entry.type ?? "file";
    const content = Buffer.from(type === "file" ? (entry.content ?? "") : "");
    const mode =
      entry.mode ??
      (type === "directory" ? 0o755 : type === "symlink" ? 0o777 : 0o644);
    blocks.push(tarHeader(entry, mode, content.length));
    if (content.length > 0) {
      blocks.push(content);
      const padding = (512 - (content.length % 512)) % 512;
      if (padding > 0) blocks.push(Buffer.alloc(padding));
    }
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

function tarHeader(entry: TarFixtureEntry, mode: number, size: number): Buffer {
  const header = Buffer.alloc(512);
  header.write(entry.name, 0, 100, "ascii");
  writeTarOctal(header, 100, 8, mode);
  writeTarOctal(header, 108, 8, 0);
  writeTarOctal(header, 116, 8, 0);
  writeTarOctal(header, 124, 12, size);
  writeTarOctal(header, 136, 12, 0);
  header[156] =
    entry.type === "directory" ? 0x35 : entry.type === "symlink" ? 0x32 : 0x30;
  if (entry.linkName !== undefined) {
    header.write(entry.linkName, 157, 100, "ascii");
  }
  header.write("ustar", 257, 6, "ascii");
  header.write("00", 263, 2, "ascii");
  header.fill(0x20, 148, 156);
  let sum = 0;
  for (const byte of header) sum += byte;
  header.write(sum.toString(8).padStart(6, "0"), 148, 6, "ascii");
  header[154] = 0;
  header[155] = 0x20;
  return header;
}

function writeTarOctal(
  header: Buffer,
  offset: number,
  length: number,
  value: number,
): void {
  const text = value.toString(8).padStart(length - 1, "0");
  header.write(text, offset, text.length, "ascii");
  header[offset + length - 1] = 0;
}
