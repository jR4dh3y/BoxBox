import { DesktopApp } from "./app";
import {
  acquireInstance,
  instanceDirectory,
  launchRequest,
  type InstanceOwner,
  type InstanceRequest,
} from "./instance";
import { launcherArguments, parseLaunchTarget } from "./launch";

function fail(stage: string, error: unknown): void {
  console.error(`BoxBox Desktop failed to ${stage}:`, error);
  process.exitCode = 1;
}

async function main(): Promise<void> {
  let request: InstanceRequest;
  try {
    const args = launcherArguments(process.platform === "linux");
    const portal = args.length === 1 && args[0] === "--portal";
    const target = portal ? undefined : parseLaunchTarget(args);
    request = portal
      ? { kind: "portal" }
      : launchRequest(target, process.env.BOXBOX_DESKTOP_URL);
  } catch (error) {
    return fail("start", error);
  }

  let instance: InstanceOwner | undefined;
  if (process.platform === "linux") {
    try {
      const result = await acquireInstance(instanceDirectory(), request);
      if (result.kind === "forwarded") return;
      instance = result.instance;
    } catch (error) {
      return fail("acquire its instance", error);
    }
  }

  let app: DesktopApp;
  try {
    app = new DesktopApp(await import("electrobun/main"), instance);
  } catch (error) {
    await instance?.close().catch(() => undefined);
    return fail("start", error);
  }

  const quit = () => void app.quit();
  process.on("SIGINT", quit);
  process.on("SIGTERM", quit);
  if (process.env.BOXBOX_DESKTOP_MANAGED_LAUNCH === "1") {
    // The managed launcher closes stdin when it is told to stop.
    process.stdin.once("end", quit);
    process.stdin.once("close", quit);
    process.stdin.resume();
  }

  try {
    await app.start(request);
  } catch (error) {
    console.error("BoxBox Desktop failed to start:", error);
    await app.quit(1);
  }
}

await main();
