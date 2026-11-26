// Launches the built desktop app on a throwaway root and passes once it has
// painted its first frame and that root's listing. Usage: desktop-smoke.ts <app>
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Never run longer than this, whatever the app or the display does.
const hardLimitMs = 90_000;

const signalApp = (
  child: ReturnType<typeof Bun.spawn>,
  grouped: boolean,
  signal: NodeJS.Signals,
) => {
  try {
    if (grouped) process.kill(-child.pid, signal);
    else child.kill(signal);
  } catch {
    // Already gone.
  }
};

async function stop(child: ReturnType<typeof Bun.spawn>, grouped: boolean) {
  signalApp(child, grouped, "SIGTERM");
  const exited = await Promise.race([
    child.exited.then(() => true),
    Bun.sleep(5000).then(() => false),
  ]);
  // Even when the leader exited at once, a child of its own may still run.
  if (!exited || grouped) signalApp(child, grouped, "SIGKILL");
  await Promise.race([child.exited, Bun.sleep(5000)]);
}

const app = process.argv[2];
if (!app) throw new Error("Usage: bun scripts/desktop-smoke.ts <app>");
const work = await mkdtemp(join(tmpdir(), "boxbox-smoke-"));
try {
  const root = join(work, "root");
  await mkdir(join(root, "folder"), { recursive: true });
  await writeFile(join(root, "note.txt"), "smoke\n");
  const trace = join(work, "trace.json");
  const env = Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !key.startsWith("BOXBOX_")),
  );
  // The app runs in its own process group, so it can be stopped as a whole. An
  // AppImage's runtime starts the real app as a child of its own; stopping only
  // the runtime leaves that child running with the output pipes open, and the
  // CI step then waits on them forever.
  const grouped = process.platform !== "win32";
  // BOXBOX_ROOT names its drive `home`; it also keys the single-instance
  // socket, so a BoxBox window already open does not take this launch.
  const child = Bun.spawn([app, "--", root], {
    detached: grouped,
    env: {
      ...env,
      BOXBOX_PROFILE: trace,
      BOXBOX_ROOT: root,
      XDG_CONFIG_HOME: join(work, "config"),
      XDG_CACHE_HOME: join(work, "cache"),
      XDG_STATE_HOME: join(work, "state"),
    },
    stdout: "inherit",
    stderr: "inherit",
  });
  const watchdog = setTimeout(() => {
    console.error(
      `desktop-smoke ran longer than ${hardLimitMs / 1000}s; stopping the app`,
    );
    signalApp(child, grouped, "SIGKILL");
    process.exit(1);
  }, hardLimitMs);
  const wanted = new Set(["startup", "open folder"]);
  const deadline = Date.now() + 60_000;
  try {
    while (wanted.size > 0) {
      if (child.exitCode !== null)
        throw new Error(`App exited with ${child.exitCode} before it painted`);
      if (Date.now() > deadline)
        throw new Error(`Never painted: ${[...wanted].join(", ")}`);
      await Bun.sleep(200);
      const text = await readFile(trace, "utf8").catch(() => "");
      for (const line of text.split("\n")) {
        // The writer ends each event with a comma; a line without one is still being written.
        if (!line.startsWith("{") || !line.endsWith(",")) continue;
        const event: unknown = JSON.parse(line.replace(/,$/, ""));
        if (
          typeof event === "object" &&
          event !== null &&
          "ph" in event &&
          event.ph === "e" &&
          "name" in event &&
          typeof event.name === "string"
        ) {
          if (event.name === "open folder: failed")
            throw new Error("The app painted a folder error instead of the listing");
          wanted.delete(event.name);
        }
      }
    }
    console.log("PASS: desktop app painted its first frame and a listed folder");
  } finally {
    clearTimeout(watchdog);
    await stop(child, grouped);
  }
} finally {
  await rm(work, { recursive: true, force: true });
}
