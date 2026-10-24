import { spawn } from "node:child_process";
import { join, resolve } from "node:path";
import {
  forwardToRunningInstance,
  instanceDirectory,
  launchRequest,
  tryAcquireLaunchGuard,
} from "./instance";
import { parseLaunchTarget, targetArguments } from "./launch";

// Electrobun initializes CEF on its native thread before loading app code.
// Arbitration must happen in this separate process, before its launcher runs.
const target = parseLaunchTarget(process.argv.slice(2));
const request = launchRequest(target, process.env.BOXBOX_DESKTOP_URL);
const directory = instanceDirectory();
const deadline = Date.now() + 30_000;
let launched = false;

while (Date.now() < deadline) {
  if (await forwardToRunningInstance(directory, request)) {
    launched = true;
    break;
  }
  const guard = await tryAcquireLaunchGuard(directory);
  if (!guard) {
    await Bun.sleep(50);
    continue;
  }
  try {
    if (await forwardToRunningInstance(directory, request)) {
      launched = true;
      break;
    }
    await runNative(
      resolve(import.meta.dir, "../bin/electrobun-launcher"),
      target ? targetArguments(target) : [],
    );
    launched = true;
    break;
  } finally {
    await guard.close();
  }
}
if (!launched) throw new Error("The running BoxBox Desktop did not become ready");

async function runNative(executable: string, args: string[]): Promise<void> {
  const child = spawn(executable, args, {
    env: { ...process.env, BOXBOX_DESKTOP_MANAGED_LAUNCH: "1" },
    cwd: join(import.meta.dir, "../bin"),
    stdio: ["pipe", "inherit", "inherit"],
    detached: true,
  });
  let timer: ReturnType<typeof setTimeout> | undefined;
  const shutdown = () => {
    child.stdin.end();
    timer ??= setTimeout(() => {
      if (child.pid !== undefined && child.exitCode === null) {
        try { process.kill(-child.pid, "SIGKILL"); } catch { /* Already exited. */ }
      }
    }, 5000);
  };
  child.stdin.on("error", () => undefined);
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
  try {
    const code = await new Promise<number>((resolve, reject) => {
      child.once("error", reject);
      child.once("exit", (status) => resolve(status ?? 1));
    });
    process.exitCode = code;
  } finally {
    clearTimeout(timer);
    process.off("SIGINT", shutdown);
    process.off("SIGTERM", shutdown);
  }
}
