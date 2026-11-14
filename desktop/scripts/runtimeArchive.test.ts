import { afterEach, describe, expect, test } from "bun:test";
import {
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readlink,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expandRuntimeArchive } from "./runtimeArchive";
import { createTar, type TarFixtureEntry } from "./tar-fixture";

const temporaryRoots: string[] = [];

afterEach(async () => {
  await Promise.all(
    temporaryRoots.splice(0).map((root) =>
      rm(root, { recursive: true, force: true }),
    ),
  );
});

async function temporaryRoot(label: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), `boxbox-${label}-`));
  temporaryRoots.push(root);
  return root;
}

function runtimeEntries(
  extra: readonly TarFixtureEntry[] = [],
): TarFixtureEntry[] {
  return [
    { name: "BoxBox/", type: "directory" },
    { name: "BoxBox/bin/", type: "directory" },
    { name: "BoxBox/bin/launcher", content: "runtime", mode: 0o750 },
    { name: "BoxBox/bin/cef/", type: "directory" },
    { name: "BoxBox/bin/cef/libcef.so", content: "cef" },
    { name: "BoxBox/bin/libcef.so", type: "symlink", linkName: "cef/libcef.so" },
    { name: "BoxBox/Resources/", type: "directory" },
    { name: "BoxBox/Resources/app/", type: "directory" },
    {
      name: "BoxBox/Resources/app/boxbox-server",
      content: "server",
      mode: 0o755,
    },
    ...extra,
  ];
}

async function writeArchive(
  root: string,
  entries: readonly TarFixtureEntry[],
): Promise<string> {
  const path = join(root, "runtime.tar.zst");
  await writeFile(path, Bun.zstdCompressSync(createTar(entries)));
  return path;
}

describe("runtime archive", () => {
  test("extracts files, directories and safe links with their modes", async () => {
    const root = await temporaryRoot("archive");
    const stage = join(root, "stage");
    await mkdir(stage);
    const archive = await writeArchive(root, runtimeEntries());

    const runtime = await expandRuntimeArchive(archive, stage);

    expect(runtime).toBe(join(stage, "BoxBox"));
    expect(await readFile(join(runtime, "bin", "launcher"), "utf8")).toBe(
      "runtime",
    );
    expect((await lstat(join(runtime, "bin", "launcher"))).mode & 0o777).toBe(
      0o750,
    );
    expect(
      await readFile(join(runtime, "Resources", "app", "boxbox-server"), "utf8"),
    ).toBe("server");
    expect(await readlink(join(runtime, "bin", "libcef.so"))).toBe(
      "cef/libcef.so",
    );
    expect(await readFile(join(runtime, "bin", "libcef.so"), "utf8")).toBe(
      "cef",
    );
  });

  test("rejects paths that leave BoxBox or the stage", async () => {
    const root = await temporaryRoot("archive-paths");
    const unsafe = [
      { name: "BoxBox/../escape.txt", content: "no" },
      { name: "/tmp/boxbox-escape.txt", content: "no" },
      { name: "Other/bin/launcher", content: "no" },
    ];
    for (const entry of unsafe) {
      const stage = join(root, "stage");
      await mkdir(stage, { recursive: true });
      const archive = await writeArchive(
        await temporaryRoot("archive-path"),
        runtimeEntries([entry]),
      );
      await expect(expandRuntimeArchive(archive, stage)).rejects.toThrow(
        /Unsafe archive/,
      );
      expect(await Bun.file(join(stage, "escape.txt")).exists()).toBe(false);
      expect(await Bun.file(join(stage, "Other", "bin", "launcher")).exists()).toBe(
        false,
      );
      await rm(stage, { recursive: true, force: true });
    }
  });

  test("rejects links that escape the runtime", async () => {
    const root = await temporaryRoot("archive-links");
    for (const linkName of ["../../escape.txt", "/etc/passwd", "../../../x"]) {
      const stage = join(root, "stage");
      await mkdir(stage, { recursive: true });
      const archive = await writeArchive(
        await temporaryRoot("archive-link"),
        runtimeEntries([
          { name: "BoxBox/bin/escape", type: "symlink", linkName },
        ]),
      );
      await expect(expandRuntimeArchive(archive, stage)).rejects.toThrow(
        /Unsafe archive link|escapes the runtime/,
      );
      await rm(stage, { recursive: true, force: true });
    }
  });

  test("rejects duplicate entries and archives without a BoxBox root", async () => {
    const root = await temporaryRoot("archive-shape");
    const stage = join(root, "stage");
    await mkdir(stage);
    const duplicate = await writeArchive(
      await temporaryRoot("archive-duplicate"),
      runtimeEntries([{ name: "BoxBox/bin/launcher", content: "again" }]),
    );
    await expect(expandRuntimeArchive(duplicate, stage)).rejects.toThrow(
      "duplicate entry",
    );

    const missingRoot = await writeArchive(
      await temporaryRoot("archive-root"),
      [{ name: "Other/", type: "directory" }],
    );
    await expect(expandRuntimeArchive(missingRoot, stage)).rejects.toThrow(
      /Unsafe archive path/,
    );
  });
});
