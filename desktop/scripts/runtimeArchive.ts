import {
  chmodSync,
  closeSync,
  createReadStream,
  createWriteStream,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  symlinkSync,
  writeSync,
} from "node:fs";
import { rm } from "node:fs/promises";
import { dirname, join, posix, sep } from "node:path";
import { pipeline } from "node:stream/promises";
import { createZstdDecompress } from "node:zlib";

const TAR_BLOCK = 512;

/**
 * Expands the trusted Electrobun runtime tarball into `stageRoot` and returns
 * the runtime directory.
 *
 * The archive is never allowed to choose where files go: every entry must stay
 * under `BoxBox/`, and links may only point inside that directory.
 */
export async function expandRuntimeArchive(
  archive: string,
  stageRoot: string,
): Promise<string> {
  const tarPath = join(stageRoot, "payload.tar");
  try {
    await pipeline(
      createReadStream(archive),
      createZstdDecompress(),
      createWriteStream(tarPath, { mode: 0o600 }),
    );
  } catch (error) {
    throw new Error(`Could not expand the BoxBox runtime archive: ${archive}`, {
      cause: error,
    });
  }
  try {
    extractRuntimeArchive(tarPath, stageRoot);
  } finally {
    await rm(tarPath, { force: true }).catch(() => {
      // The staged runtime is still usable; the caller removes the stage
      // directory if the install fails.
    });
  }
  return join(stageRoot, "BoxBox");
}

function extractRuntimeArchive(tarPath: string, destination: string): void {
  const descriptor = openSync(tarPath, "r");
  const names = new Set<string>();
  const links: { readonly name: string; readonly path: string }[] = [];
  let position = 0;
  let hasRoot = false;
  try {
    for (;;) {
      const header = Buffer.alloc(TAR_BLOCK);
      readExact(descriptor, header, position);
      if (isZeroBlock(header)) break;
      position += TAR_BLOCK;

      verifyTarChecksum(header);
      const name = tarPathName(header);
      const type = tarType(header);
      const size = tarSize(header);
      const mode = tarMode(header);
      const entryPath = archiveEntryPath(name, type === "5");
      if (names.has(entryPath)) {
        throw new Error(`Archive contains duplicate entry: ${name}`);
      }
      names.add(entryPath);
      if (links.some((link) => entryPath.startsWith(`${link.name}/`))) {
        throw new Error(`Archive entry is nested under a link: ${name}`);
      }
      if (entryPath === "BoxBox") {
        if (type !== "5") {
          throw new Error("The runtime archive root must be a directory");
        }
        hasRoot = true;
      }

      const target = join(destination, ...entryPath.split("/"));
      if (type === "5") {
        mkdirSync(target, { recursive: true, mode: 0o755 });
        chmodSync(target, 0o755);
      } else if (type === "0") {
        if (entryPath === "BoxBox") {
          throw new Error("The runtime archive root must be a directory");
        }
        mkdirSync(dirname(target), { recursive: true, mode: 0o755 });
        writeTarFile(descriptor, target, position, size, mode);
      } else if (type === "2") {
        if (entryPath === "BoxBox") {
          throw new Error("The runtime archive root must be a directory");
        }
        const linkName = tarLinkName(header);
        validateLinkTarget(entryPath, linkName);
        mkdirSync(dirname(target), { recursive: true, mode: 0o755 });
        symlinkSync(linkName, target);
        links.push({ name: entryPath, path: target });
      } else {
        throw new Error(`Unsupported archive entry type "${type}": ${name}`);
      }

      // Directory entries normally have no data, but skipping the declared
      // size keeps the block offset correct for every valid tar.
      position += Math.ceil(size / TAR_BLOCK) * TAR_BLOCK;
    }
  } finally {
    closeSync(descriptor);
  }

  if (!hasRoot) {
    throw new Error("The runtime archive does not contain a BoxBox directory");
  }
  const root = join(destination, "BoxBox");
  const realRoot = realpathSync(root);
  for (const link of links) {
    const real = realpathSync(link.path);
    if (real !== realRoot && !real.startsWith(realRoot + sep)) {
      throw new Error(`Archive link escapes the runtime: ${link.name}`);
    }
  }
}

function archiveEntryPath(raw: string, directory: boolean): string {
  const value = directory ? raw.replace(/\/+$/, "") : raw;
  if (
    value.length === 0 ||
    value.startsWith("/") ||
    value.includes("\0") ||
    value.includes("\n")
  ) {
    throw new Error(`Unsafe archive path: ${JSON.stringify(raw)}`);
  }
  const parts = value.split("/");
  if (
    parts.some((part) => part === "" || part === "." || part === "..") ||
    parts[0] !== "BoxBox"
  ) {
    throw new Error(`Unsafe archive path: ${JSON.stringify(raw)}`);
  }
  return parts.join("/");
}

function validateLinkTarget(entryPath: string, linkName: string): void {
  if (
    linkName.length === 0 ||
    linkName.includes("\0") ||
    linkName.startsWith("/")
  ) {
    throw new Error(`Unsafe archive link: ${entryPath}`);
  }
  const target = posix.normalize(
    posix.join(posix.dirname(entryPath), linkName),
  );
  const parts = target.split("/");
  if (parts[0] !== "BoxBox" || parts.includes("..")) {
    throw new Error(`Archive link escapes the runtime: ${entryPath}`);
  }
}

function writeTarFile(
  source: number,
  target: string,
  offset: number,
  size: number,
  mode: number,
): void {
  const destination = openSync(target, "wx", mode | 0o600);
  try {
    const buffer = Buffer.alloc(Math.min(size, 1024 * 1024));
    let remaining = size;
    let position = offset;
    while (remaining > 0) {
      const length = Math.min(remaining, buffer.length);
      readExact(source, buffer, position, length);
      writeExact(destination, buffer, length);
      position += length;
      remaining -= length;
    }
  } finally {
    closeSync(destination);
  }
  chmodSync(target, mode & 0o777);
}

function readExact(
  descriptor: number,
  buffer: Buffer,
  position: number,
  length = buffer.length,
): void {
  let done = 0;
  while (done < length) {
    const read = readSync(
      descriptor,
      buffer,
      done,
      length - done,
      position + done,
    );
    if (read === 0) throw new Error("Unexpected end of the runtime archive");
    done += read;
  }
}

function writeExact(descriptor: number, buffer: Buffer, length: number): void {
  let done = 0;
  while (done < length) {
    const written = writeSync(descriptor, buffer, done, length - done);
    if (written <= 0) throw new Error("Could not write the extracted runtime");
    done += written;
  }
}

function isZeroBlock(block: Buffer): boolean {
  return block.every((byte) => byte === 0);
}

function verifyTarChecksum(header: Buffer): void {
  const stored = tarNumber(header, 148, 8);
  let sum = 0;
  for (let index = 0; index < header.length; index += 1) {
    sum += index >= 148 && index < 156 ? 32 : (header[index] ?? 0);
  }
  if (sum !== stored) {
    throw new Error("The runtime archive has a corrupt header");
  }
}

function tarPathName(header: Buffer): string {
  const name = tarText(header, 0, 100);
  const prefix = tarText(header, 345, 155);
  return prefix.length > 0 ? `${prefix}/${name}` : name;
}

function tarType(header: Buffer): string {
  const value = header[156] ?? 0;
  return value === 0 ? "0" : String.fromCharCode(value);
}

function tarSize(header: Buffer): number {
  return tarNumber(header, 124, 12);
}

function tarMode(header: Buffer): number {
  return tarNumber(header, 100, 8) & 0o777;
}

function tarLinkName(header: Buffer): string {
  return tarText(header, 157, 100);
}

function tarText(header: Buffer, offset: number, length: number): string {
  const field = header.subarray(offset, offset + length);
  const end = field.indexOf(0);
  return field.subarray(0, end < 0 ? field.length : end).toString("utf8");
}

function tarNumber(header: Buffer, offset: number, length: number): number {
  const text = tarText(header, offset, length).trim();
  if (text === "") return 0;
  if (!/^[0-7]+$/.test(text)) {
    throw new Error("The runtime archive has an invalid header");
  }
  return Number.parseInt(text, 8);
}
