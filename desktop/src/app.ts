import type { BrowserWindow } from "electrobun/main";
import { join } from "node:path";
import { ConnectionsStore } from "./connections";
import { isConnectionsHostMessage, localBootstrapURL } from "./connections-host";
import type { ConnectionsRPC } from "./connections-rpc";
import type { InstanceOwner, InstanceRequest } from "./instance";
import type { LaunchTarget } from "./launch";
import { LocalServer } from "./server";
import { remoteURL } from "./target";

type Electrobun = typeof import("electrobun/main");

const CONNECTIONS_SHORTCUT = "CommandOrControl+Shift+O";
const BROWSER_FRAME = { width: 1280, height: 800 };

/** Owns the windows and the local server. Closing the last window quits. */
export class DesktopApp {
  readonly #sdk: Electrobun;
  readonly #instance: InstanceOwner | undefined;
  readonly #connections: ConnectionsStore;
  readonly #server: LocalServer;
  readonly #windows = new Set<number>();
  /** One window per remote URL, whether a saved profile or a launch opened it. */
  readonly #remoteWindows = new Map<string, number>();
  #localWindowId: number | undefined;
  #managerWindowId: number | undefined;
  #primaryWindowId: number | undefined;
  #quitting = false;

  constructor(sdk: Electrobun, instance: InstanceOwner | undefined) {
    this.#sdk = sdk;
    this.#instance = instance;
    this.#connections = new ConnectionsStore(sdk.Utils.paths.userData);
    this.#server = new LocalServer(
      join(sdk.PATHS.RESOURCES_FOLDER, "app", "boxbox-server"),
      sdk.Utils.paths.userData,
      (code) => {
        console.error(`Local BoxBox server exited (${code})`);
        void this.quit(1);
      },
    );
    sdk.default.events.on("before-quit", (event: { response: { allow: boolean } }) => {
      if (this.#quitting) return;
      event.response = { allow: false };
      void this.quit();
    });
  }

  async start(request: InstanceRequest): Promise<void> {
    const build = await this.#sdk.BuildConfig.get();
    if (!build.availableRenderers.includes("cef")) {
      throw new Error("BoxBox Desktop requires a CEF build");
    }
    const registered = this.#sdk.GlobalShortcut.register(CONNECTIONS_SHORTCUT, () =>
      this.#openManager(),
    );
    if (!registered) {
      console.warn(`Could not register ${CONNECTIONS_SHORTCUT}; use Connections in BoxBox.`);
    }

    const primary =
      request.kind === "remote"
        ? this.#openRemote(request.url, "BoxBox — Remote")
        : await this.#openLocal(request.kind === "local" ? request.target : undefined);
    this.#primaryWindowId = primary.id;
    this.#instance?.setRequestHandler((next) => this.#handle(next));
  }

  async quit(code = 0): Promise<void> {
    if (this.#quitting) return;
    this.#quitting = true;
    this.#sdk.GlobalShortcut.unregisterAll();
    await this.#server.stop().catch((error) => {
      console.error("Could not stop the local BoxBox server:", error);
    });
    await this.#instance?.close().catch((error) => {
      console.error("Could not close the BoxBox Desktop instance socket:", error);
    });
    // Electrobun begins native/CEF shutdown synchronously from this call.
    this.#sdk.Utils.quit(code);
  }

  async #handle(request: InstanceRequest): Promise<void> {
    if (this.#quitting) return;
    switch (request.kind) {
      case "activate":
        this.#focusPrimary();
        return;
      case "local":
        await this.#openLocal(request.target);
        return;
      case "remote":
        this.#openRemote(request.url, "BoxBox — Remote");
        return;
    }
  }

  #window(id: number | undefined): BrowserWindow | undefined {
    return id === undefined ? undefined : this.#sdk.BrowserWindow.getById(id);
  }

  #track(window: BrowserWindow, onClose: () => void): void {
    this.#windows.add(window.id);
    window.on("close", () => {
      if (!this.#windows.delete(window.id)) return;
      onClose();
      if (!this.#quitting && this.#windows.size === 0) void this.quit();
    });
  }

  #focusPrimary(): void {
    for (const id of [this.#primaryWindowId, ...this.#windows]) {
      const window = this.#window(id);
      if (!window) continue;
      window.show();
      return;
    }
  }

  async #openLocal(target?: LaunchTarget): Promise<BrowserWindow> {
    if (this.#quitting) throw new Error("BoxBox Desktop is shutting down");
    const server = await this.#server.get();
    const existing = this.#window(this.#localWindowId);
    if (existing) {
      existing.show();
      if (target) existing.webview.loadURL(localBootstrapURL(server.url, target));
      return existing;
    }

    const window = new this.#sdk.BrowserWindow({
      title: "BoxBox",
      url: localBootstrapURL(server.url, target),
      renderer: "cef",
      sandbox: true,
      allowedProtocols: { views: false, appData: false },
      frame: BROWSER_FRAME,
    });
    this.#localWindowId = window.id;
    const messageEvent = `host-message-${window.webview.id}`;
    const onHostMessage = (event: unknown) => {
      if (isConnectionsHostMessage(event)) this.#openManager();
    };
    this.#sdk.default.events.on(messageEvent, onHostMessage);
    this.#track(window, () => {
      this.#sdk.default.events.off(messageEvent, onHostMessage);
      if (this.#localWindowId === window.id) this.#localWindowId = undefined;
    });
    return window;
  }

  #openRemote(value: string, title: string): BrowserWindow {
    if (this.#quitting) throw new Error("BoxBox Desktop is shutting down");
    const url = remoteURL(value);
    const existing = this.#window(this.#remoteWindows.get(url));
    if (existing) {
      existing.show();
      return existing;
    }

    const window = new this.#sdk.BrowserWindow({
      title,
      url,
      renderer: "cef",
      sandbox: true,
      allowedProtocols: { views: false, appData: false },
      frame: BROWSER_FRAME,
    });
    this.#remoteWindows.set(url, window.id);
    this.#track(window, () => {
      if (this.#remoteWindows.get(url) === window.id) this.#remoteWindows.delete(url);
    });
    return window;
  }

  #openManager(): void {
    if (this.#quitting) return;
    const existing = this.#window(this.#managerWindowId);
    if (existing) {
      existing.show();
      return;
    }

    const rpc = this.#sdk.BrowserView.defineRPC<ConnectionsRPC>({
      handlers: {
        requests: {
          listConnections: () => this.#connections.list(),
          saveConnection: (command) => this.#connections.save(command),
          removeConnection: (params) => this.#connections.remove(params),
          connectProfile: async (params) => {
            const profile = await this.#connections.get(params);
            this.#openRemote(profile.url, `BoxBox — ${profile.name}`);
          },
          showLocalFiles: async () => {
            await this.#openLocal();
          },
        },
      },
    });
    const window = new this.#sdk.BrowserWindow({
      title: "BoxBox Connections",
      url: "views://connection-manager/index.html",
      renderer: "cef",
      sandbox: false,
      allowedProtocols: { views: true, appData: false },
      navigationRules: JSON.stringify(["views://connection-manager/*"]),
      rpc,
      frame: { width: 820, height: 650 },
    });
    this.#managerWindowId = window.id;
    this.#track(window, () => {
      if (this.#managerWindowId === window.id) this.#managerWindowId = undefined;
    });
  }
}
