import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { installDesktopIntegration } from "./install";

if (!process.env.DISPLAY && !process.env.WAYLAND_DISPLAY) {
  throw new Error("Native smoke requires a Linux graphical session");
}

const fixture = await mkdtemp(join(tmpdir(), "boxbox-native-smoke-"));
const runtime = join(fixture, "runtime");
await mkdir(runtime, { mode: 0o700 });
const env = {
  ...process.env,
  BOXBOX_DESKTOP_SMOKE: "1",
  HOME: fixture,
  GDK_BACKEND: "x11",
  XDG_DATA_HOME: join(fixture, "data"),
  XDG_CONFIG_HOME: join(fixture, "config"),
  XDG_CACHE_HOME: join(fixture, "cache"),
  XDG_RUNTIME_DIR: runtime,
};

function launch(executable: string, target: string) {
  let output = "";
  let applicationPid: number | undefined;
  const child = Bun.spawn([executable, "--select", target], {
    env,
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  });
  async function collect(stream: ReadableStream<Uint8Array>) {
    const decoder = new TextDecoder();
    for await (const chunk of stream) {
      output += decoder.decode(chunk, { stream: true });
      const match = /Child process spawned with PID (\d+)/.exec(output);
      if (match?.[1]) applicationPid = Number(match[1]);
      output = output.slice(-256_000);
    }
  }
  const drained = Promise.all([collect(child.stdout), collect(child.stderr)]);
  return {
    child,
    drained,
    get output() {
      return output;
    },
    get applicationPid() {
      return applicationPid;
    },
    stop(signal: "SIGTERM" | "SIGKILL" = "SIGTERM") {
      if (child.exitCode !== null) return;
      if (applicationPid !== undefined) {
        try {
          process.kill(applicationPid, signal);
        } catch {
          /* Already exited. */
        }
      }
      if (child.exitCode === null && applicationPid === undefined) child.kill();
    },
  };
}

async function until(
  check: () => boolean,
  label: string,
  details: () => string,
) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    if (check()) return;
    await Bun.sleep(100);
  }
  throw new Error(`${label}\n${details()}`);
}

let primary: ReturnType<typeof launch> | undefined;
let secondary: ReturnType<typeof launch> | undefined;
try {
  const launcher =
    process.argv[2] ?? (await installDesktopIntegration({ env })).launcher;
  // Exercise startup with a rendering preference that requires writable metadata.
  const preferences = join(
    env.XDG_DATA_HOME,
    "io.boxbox.desktop/stable/desktop",
  );
  await mkdir(preferences, { recursive: true });
  await writeFile(
    join(preferences, "preferences.json"),
    '{"hardwareAcceleration":false}',
  );
  const firstDirectory = join(fixture, "first");
  const secondDirectory = join(fixture, "second");
  await mkdir(firstDirectory);
  await mkdir(secondDirectory);
  const firstFile = join(firstDirectory, "first.txt");
  const secondFile = join(secondDirectory, "second.txt");
  await writeFile(firstFile, "first native launch");
  await writeFile(secondFile, "forwarded native launch");

  const owner = launch(launcher, firstFile);
  primary = owner;
  await until(
    () =>
      owner.output.includes(`/api/v1/_desktop-painted/root${firstDirectory}`),
    "The native window did not paint its launch folder",
    () => owner.output,
  );
  assert.equal(owner.child.exitCode, null);
  assert.ok(owner.output.includes("CEF EVENT LOOP STARTED"));
  assert.ok(
    owner.applicationPid,
    "The native launcher reported an application PID",
  );
  const environment = await readFile(
    `/proc/${owner.applicationPid}/environ`,
    "utf8",
  );
  assert.ok(
    environment.split("\0").includes("BOXBOX_HARDWARE_ACCELERATION=false"),
    "The saved rendering preference reaches the running application",
  );

  const forwarded = launch(launcher, secondFile);
  secondary = forwarded;
  await until(
    () => forwarded.child.exitCode !== null,
    "Second launch did not exit",
    () => forwarded.output,
  );
  await forwarded.drained;
  assert.equal(forwarded.child.exitCode, 0, forwarded.output);
  assert.equal(
    forwarded.output.includes("CEF EVENT LOOP STARTED"),
    false,
    forwarded.output,
  );
  await until(
    () =>
      owner.output.includes(`/api/v1/_desktop-painted/root${secondDirectory}`),
    "The running window did not paint the forwarded folder",
    () => owner.output,
  );
  assert.equal(owner.child.exitCode, null);
  owner.stop();
  await until(
    () => owner.child.exitCode !== null,
    "Native shutdown timed out",
    () => owner.output,
  );
  await owner.drained;
  assert.ok(owner.output.includes("Server shutdown complete"), owner.output);
  console.log(
    "Native smoke passed: isolated runtime install, CEF folder launch, second-launch handoff without CEF, backend shutdown.",
  );
} finally {
  for (const running of [primary, secondary]) {
    if (!running) continue;
    running.stop();
    let timer: ReturnType<typeof setTimeout> | undefined;
    await Promise.race([
      running.child.exited,
      new Promise<void>((resolve) => {
        timer = setTimeout(resolve, 5000);
      }),
    ]).finally(() => clearTimeout(timer));
    if (running.child.exitCode === null) {
      running.stop("SIGKILL");
      running.child.kill("SIGKILL");
      await running.child.exited;
    }
    await running.drained;
  }
  await rm(fixture, { recursive: true, force: true });
}
