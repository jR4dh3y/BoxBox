import type { BrowserWindow as BrowserWindowType } from "electrobun/main";
import { join } from "node:path";
import { ConnectionsStore } from "./connections";
import { isConnectionsHostMessage, localBootstrapURL } from "./connections-host";
import type { ConnectionsRPC } from "./connections-rpc";
import {
  launcherArguments,
  parseLaunchTarget,
  type LaunchTarget,
} from "./launch";
import {
  acquireInstance,
  instanceDirectory,
  type InstanceOwner,
  type InstanceRequest,
} from "./instance";
import { startServer } from "./server";
import { remoteURL } from "./target";

type ElectrobunMain = typeof import("electrobun/main");
type LocalServer = Awaited<ReturnType<typeof startServer>>;
type TrackedRemoteWindow = { readonly windowId: number; readonly url: string };

const openWindowIds = new Set<number>();
const remoteWindows = new Map<string, TrackedRemoteWindow>();
const requestedRemoteWindows = new Map<string, TrackedRemoteWindow>();
const connectionsShortcut = "CommandOrControl+Shift+O";

let electrobun: ElectrobunMain | undefined;
let connections: ConnectionsStore;
let localServer: LocalServer | undefined;
let localServerStarting: Promise<LocalServer> | undefined;
let localWindowId: number | undefined;
let managerWindowId: number | undefined;
let primaryWindowId: number | undefined;
let localHostMessageEvent: string | undefined;
let localHostMessageHandler: ((event: unknown) => void) | undefined;
let initialLaunchTarget: LaunchTarget | undefined;
let initialRemoteURL: string | undefined;
let instance: InstanceOwner | undefined;
let quitting = false;

function sdk(): ElectrobunMain {
  if (!electrobun) throw new Error("Electrobun has not been initialized");
  return electrobun;
}

async function quit(code = 0): Promise<void> {
  if (quitting) return;
  quitting = true;
  electrobun?.GlobalShortcut.unregisterAll();
  if (localServerStarting) await localServerStarting.catch(() => undefined);
  await localServer?.stop().catch((error) => {
    console.error("Could not stop the local BoxBox server:", error);
  });
  await instance?.close().catch((error) => {
    console.error("Could not close the BoxBox Desktop instance socket:", error);
  });
  // Electrobun begins native/CEF shutdown synchronously from this call.
  if (electrobun) electrobun.Utils.quit(code);
  else process.exitCode = code;
}

function trackWindow(
  window: BrowserWindowType,
  onClose: () => void = () => undefined,
): void {
  openWindowIds.add(window.id);
  window.on("close", () => {
    if (!openWindowIds.delete(window.id)) return;
    onClose();
    if (!quitting && openWindowIds.size === 0) void quit();
  });
}

function requireOpenApplication(): void {
  if (quitting) throw new Error("BoxBox Desktop is shutting down");
}

function ensureLocalServer(): Promise<LocalServer> {
  requireOpenApplication();
  if (localServer) return Promise.resolve(localServer);
  localServerStarting ??= startServer(
    join(sdk().PATHS.RESOURCES_FOLDER, "app", "boxbox-server"),
    sdk().Utils.paths.userData,
  )
    .then((started) => {
      localServer = started;
      void started.exited.then((code) => {
        if (!quitting) {
          console.error(`Local BoxBox server exited (${code})`);
          void quit(1);
        }
      });
      return started;
    })
    .finally(() => {
      localServerStarting = undefined;
    });
  return localServerStarting;
}

function openRemoteWindow(urlValue: string, title: string): BrowserWindowType {
  requireOpenApplication();
  const url = remoteURL(urlValue);
  const window = new (sdk().BrowserWindow)({
    title,
    url,
    renderer: "cef",
    sandbox: true,
    allowedProtocols: { views: false, appData: false },
    frame: { width: 1280, height: 800 },
  });
  trackWindow(window);
  window.on("close", () => {
    if (requestedRemoteWindows.get(url)?.windowId === window.id) {
      requestedRemoteWindows.delete(url);
    }
  });
  return window;
}

async function openSavedConnection(value: unknown): Promise<void> {
  const profile = await connections.get(value);
  const existing = remoteWindows.get(profile.id);
  if (existing) {
    const existingWindow = sdk().BrowserWindow.getById(existing.windowId);
    if (existingWindow && existing.url === profile.url) {
      existingWindow.show();
      return;
    }
    existingWindow?.close();
    if (remoteWindows.get(profile.id)?.windowId === existing.windowId) {
      remoteWindows.delete(profile.id);
    }
  }

  const requested = requestedRemoteWindows.get(profile.url);
  if (requested) {
    const existingWindow = sdk().BrowserWindow.getById(requested.windowId);
    if (existingWindow) {
      existingWindow.show();
      remoteWindows.set(profile.id, requested);
      return;
    }
  }

  const window = openRemoteWindow(profile.url, `BoxBox — ${profile.name}`);
  const tracked: TrackedRemoteWindow = { windowId: window.id, url: profile.url };
  remoteWindows.set(profile.id, tracked);
  window.on("close", () => {
    if (remoteWindows.get(profile.id)?.windowId === window.id) {
      remoteWindows.delete(profile.id);
    }
  });
}

function openRemoteRequest(urlValue: string): BrowserWindowType {
  const url = remoteURL(urlValue);
  const requested = requestedRemoteWindows.get(url);
  const requestedWindow = requested && sdk().BrowserWindow.getById(requested.windowId);
  if (requestedWindow) {
    requestedWindow.show();
    return requestedWindow;
  }

  for (const tracked of remoteWindows.values()) {
    if (tracked.url !== url) continue;
    const existing = sdk().BrowserWindow.getById(tracked.windowId);
    if (existing) {
      existing.show();
      requestedRemoteWindows.set(url, tracked);
      return existing;
    }
  }

  const window = openRemoteWindow(url, "BoxBox — Remote");
  requestedRemoteWindows.set(url, { windowId: window.id, url });
  return window;
}

function focusPrimaryWindow(): void {
  const candidates = primaryWindowId === undefined
    ? [...openWindowIds]
    : [primaryWindowId, ...openWindowIds];
  for (const id of candidates) {
    const window = sdk().BrowserWindow.getById(id);
    if (!window) continue;
    window.show();
    return;
  }
}

function handleInstanceRequest(request: InstanceRequest): Promise<void> | void {
  if (quitting) return;
  switch (request.kind) {
    case "activate":
      focusPrimaryWindow();
      return;
    case "local":
      return openLocalFiles(request.target);
    case "remote":
      openRemoteRequest(request.url);
      return;
  }
}

function managerRPC() {
  return sdk().BrowserView.defineRPC<ConnectionsRPC>({
    handlers: {
      requests: {
        listConnections: () => connections.list(),
        saveConnection: (command) => connections.save(command),
        removeConnection: (params) => connections.remove(params),
        connectProfile: (params) => openSavedConnection(params),
        showLocalFiles: () => openLocalFiles(),
      },
    },
  });
}

function openConnectionsManager(): void {
  requireOpenApplication();
  const existing = managerWindowId === undefined
    ? undefined
    : sdk().BrowserWindow.getById(managerWindowId);
  if (existing) {
    existing.show();
    return;
  }

  const window = new (sdk().BrowserWindow)({
    title: "BoxBox Connections",
    url: "views://connection-manager/index.html",
    renderer: "cef",
    sandbox: false,
    allowedProtocols: { views: true, appData: false },
    navigationRules: JSON.stringify(["views://connection-manager/*"]),
    rpc: managerRPC(),
    frame: { width: 820, height: 650 },
  });
  managerWindowId = window.id;
  trackWindow(window, () => {
    if (managerWindowId === window.id) managerWindowId = undefined;
  });
}

async function openLocalFiles(target?: LaunchTarget): Promise<void> {
  requireOpenApplication();
  const server = await ensureLocalServer();
  const targetToOpen = target ?? initialLaunchTarget;
  const existing = localWindowId === undefined
    ? undefined
    : sdk().BrowserWindow.getById(localWindowId);
  if (existing) {
    existing.show();
    if (targetToOpen) {
      existing.webview.loadURL(localBootstrapURL(server.url, targetToOpen));
    }
    initialLaunchTarget = undefined;
    return;
  }

  const window = new (sdk().BrowserWindow)({
    title: "BoxBox",
    url: localBootstrapURL(server.url, targetToOpen),
    renderer: "cef",
    sandbox: true,
    allowedProtocols: { views: false, appData: false },
    frame: { width: 1280, height: 800 },
  });
  initialLaunchTarget = undefined;
  localWindowId = window.id;
  const messageEvent = `host-message-${window.webview.id}`;
  const onHostMessage = (event: unknown) => {
    if (isConnectionsHostMessage(event)) {
      try {
        openConnectionsManager();
      } catch (error) {
        console.error("Could not open the connections manager:", error);
      }
    }
  };
  localHostMessageEvent = messageEvent;
  localHostMessageHandler = onHostMessage;
  sdk().default.events.on(messageEvent, onHostMessage);
  trackWindow(window, () => {
    if (localHostMessageEvent && localHostMessageHandler) {
      sdk().default.events.off(localHostMessageEvent, localHostMessageHandler);
    }
    localHostMessageEvent = undefined;
    localHostMessageHandler = undefined;
    if (localWindowId === window.id) localWindowId = undefined;
  });
}

async function main(): Promise<void> {
  let startupRequest: InstanceRequest;
  try {
    initialLaunchTarget = parseLaunchTarget(
      launcherArguments(process.platform === "linux"),
    );
    initialRemoteURL = process.env.BOXBOX_DESKTOP_URL
      ? remoteURL(process.env.BOXBOX_DESKTOP_URL)
      : undefined;
    startupRequest = initialRemoteURL
      ? { kind: "remote", url: initialRemoteURL }
      : initialLaunchTarget
      ? { kind: "local", target: initialLaunchTarget }
      : { kind: "activate" };
  } catch (error) {
    console.error("BoxBox Desktop failed to start:", error);
    process.exitCode = 1;
    return;
  }

  if (process.platform === "linux") {
    try {
      const result = await acquireInstance(instanceDirectory(), startupRequest);
      if (result.kind === "forwarded") return;
      instance = result.instance;
    } catch (error) {
      console.error("BoxBox Desktop failed to acquire its instance:", error);
      process.exitCode = 1;
      return;
    }
  }

  try {
    electrobun = await import("electrobun/main");
    connections = new ConnectionsStore(electrobun.Utils.paths.userData);
    electrobun.default.events.on(
      "before-quit",
      (event: { response: { allow: boolean } }) => {
        if (!quitting) {
          event.response = { allow: false };
          void quit();
        }
      },
    );
    process.on("SIGINT", () => void quit());
    process.on("SIGTERM", () => void quit());
    if (process.env.BOXBOX_DESKTOP_MANAGED_LAUNCH === "1") {
      const onManagedLauncherClosed = () => void quit();
      process.stdin.once("end", onManagedLauncherClosed);
      process.stdin.once("close", onManagedLauncherClosed);
      process.stdin.resume();
    }

    const build = await electrobun.BuildConfig.get();
    if (!build.availableRenderers.includes("cef")) {
      throw new Error("BoxBox Desktop requires a CEF build");
    }
    if (!electrobun.GlobalShortcut.register(connectionsShortcut, () => {
      try {
        openConnectionsManager();
      } catch (error) {
        console.error("Could not open the connections manager:", error);
      }
    })) {
      console.warn(`Could not register ${connectionsShortcut}; use Connections in BoxBox.`);
    }

    if (initialRemoteURL) {
      primaryWindowId = openRemoteRequest(initialRemoteURL).id;
    } else {
      await openLocalFiles(initialLaunchTarget);
      primaryWindowId = localWindowId;
    }
    instance?.setRequestHandler(handleInstanceRequest);
  } catch (error) {
    console.error("BoxBox Desktop failed to start:", error);
    await quit(1);
  }
}

await main();
