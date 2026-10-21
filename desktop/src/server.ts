import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { serverURL } from "./target";

export async function startServer(binary: string, dataDir: string) {
  const workingDirectory = await mkdtemp(join(tmpdir(), "boxbox-desktop-"));
  const token = crypto.getRandomValues(new Uint8Array(32)).toHex();
  await mkdir(dataDir, { recursive: true, mode: 0o700 });
  const configPath = join(workingDirectory, "config.json");
  const home = homedir();
  const folders = [
    "Desktop",
    "Downloads",
    "Documents",
    "Music",
    "Pictures",
    "Videos",
  ];
  await writeFile(
    configPath,
    JSON.stringify({
      port: 8080,
      host: "127.0.0.1",
      data_dir: dataDir,
      allow_root_mount: true,
      mount_points: [
        { name: "root", path: "/", read_only: false },
        { name: "home", path: home, read_only: false },
        ...folders
          .filter((folder) => existsSync(join(home, folder)))
          .map((folder) => ({
            name: folder.toLowerCase(),
            path: join(home, folder),
            read_only: false,
          })),
      ],
    }),
    { mode: 0o600 },
  );

  // Deployment overrides must not change desktop mounts, origins or credentials.
  const env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) =>
        !key.startsWith("BOXBOX_") &&
        !key.startsWith("FM_") &&
        key !== "CONFIG_PATH",
    ),
  );
  const child = Bun.spawn([binary, "--desktop", "--config", configPath], {
    cwd: workingDirectory,
    env: { ...env, BOXBOX_DESKTOP_TOKEN: token },
    stdin: "pipe",
    stdout: "pipe",
    stderr: "inherit",
  });
  const onExit = () => {
    if (child.exitCode === null) child.kill("SIGTERM");
  };
  process.on("exit", onExit);
  let stopping: Promise<void> | undefined;
  function stop(): Promise<void> {
    stopping ??= (async () => {
      if (child.exitCode === null) child.kill("SIGTERM");
      const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
      try {
        await child.exited;
      } finally {
        clearTimeout(timer);
      }
      process.off("exit", onExit);
      await rm(workingDirectory, { recursive: true, force: true });
    })();
    return stopping;
  }
  try {
    const ready = (async () => {
      let buffer = "";
      for await (const chunk of child.stdout) {
        buffer += new TextDecoder().decode(chunk);
        if (buffer.length > 4096)
          throw new Error("Invalid desktop readiness message");
        if (buffer.includes("\n"))
          return serverURL(buffer.split("\n")[0] ?? "");
      }
      throw new Error("Desktop server exited before becoming ready");
    })();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(
        () => reject(new Error("Desktop server startup timed out")),
        30_000,
      );
    });
    const url = await Promise.race([ready, timeout]).finally(() =>
      clearTimeout(timer),
    );
    return {
      url: `${url}/_desktop?token=${token}`,
      stop,
      exited: child.exited,
    };
  } catch (error) {
    await stop();
    throw error;
  }
}
