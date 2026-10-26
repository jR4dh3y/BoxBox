import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
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
    env: {
      ...env, BOXBOX_DESKTOP_TOKEN: token,
      ...(existsSync(join(dirname(binary), "native/thumbnail-worker.js"))
        ? { BOXBOX_DESKTOP_THUMBNAIL_COMMAND: JSON.stringify([process.execPath, join(dirname(binary), "native/thumbnail-worker.js")]) }
        : {}),
    },
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

export type StartedServer = Awaited<ReturnType<typeof startServer>>;

/** Starts the bundled server on first use and reports an unexpected exit once. */
export class LocalServer {
  readonly #binary: string;
  readonly #dataDir: string;
  readonly #onUnexpectedExit: (code: number) => void;
  #server: Promise<StartedServer> | undefined;
  #stopping = false;

  constructor(
    binary: string,
    dataDir: string,
    onUnexpectedExit: (code: number) => void,
  ) {
    this.#binary = binary;
    this.#dataDir = dataDir;
    this.#onUnexpectedExit = onUnexpectedExit;
  }

  get(): Promise<StartedServer> {
    if (this.#stopping) return Promise.reject(new Error("BoxBox Desktop is shutting down"));
    this.#server ??= startServer(this.#binary, this.#dataDir).then((started) => {
      void started.exited.then((code) => {
        if (!this.#stopping) this.#onUnexpectedExit(code);
      });
      return started;
    });
    // A failed start may be retried by the next window.
    this.#server.catch(() => (this.#server = undefined));
    return this.#server;
  }

  async stop(): Promise<void> {
    this.#stopping = true;
    const server = await this.#server?.catch(() => undefined);
    await server?.stop();
  }
}
