import { watchChoosers, chooserBootstrap, cancelChooser } from "./chooser";
import { hardwareAccelerationEnabled } from "./rendering";
import type { BrowserWindow } from "electrobun/main";
import { join } from "node:path";
import { localBootstrapURL, remoteLaunchTarget } from "./connections-host";
import type { InstanceOwner, InstanceRequest } from "./instance";
import type { LaunchTarget } from "./launch";
import { rendererTargetScript } from "./renderer-target";
import { LocalServer } from "./server";

type Electrobun = typeof import("electrobun/main");

const BROWSER_FRAME = { width: 1280, height: 800 };

/** Owns the windows and the local server. Closing the last window quits. */
export class DesktopApp {
  readonly #sdk: Electrobun;
  readonly #instance: InstanceOwner | undefined;
  readonly #server: LocalServer;
  readonly #windows = new Set<number>();
  #localWindowId: number | undefined;
  readonly #chooserWindows = new Map<string, BrowserWindow>();
  readonly #chooserAbort = new AbortController();
  #primaryWindowId: number | undefined;
  #quitting = false;
  #localReady = false;
  #pendingTargets: LaunchTarget[] = [];

  constructor(sdk: Electrobun, instance: InstanceOwner | undefined) {
    this.#sdk = sdk;
    this.#instance = instance;
    this.#server = new LocalServer(
      join(sdk.PATHS.RESOURCES_FOLDER, "app", "boxbox-server"),
      sdk.Utils.paths.userData,
      (code) => {
        console.error(`Local BoxBox server exited (${code})`);
        void this.quit(1);
      },
    );
    sdk.default.events.on(
      "before-quit",
      (event: { response: { allow: boolean } }) => {
        if (this.#quitting) return;
        event.response = { allow: false };
        void this.quit();
      },
    );
  }

  async start(request: InstanceRequest): Promise<void> {
    const build = await this.#sdk.BuildConfig.get();
    if (!build.availableRenderers.includes("cef")) {
      throw new Error("BoxBox Desktop requires a CEF build");
    }
    const primary =
      request.kind === "portal"
        ? undefined
        : request.kind === "remote"
          ? await this.#openRemote(request.url)
          : await this.#openLocal(
              request.kind === "local" ? request.target : undefined,
            );
    this.#primaryWindowId = primary?.id;
    const server = await this.#server.get();
    void watchChoosers(server.url, this.#chooserAbort.signal, (ids) => {
      const active = new Set(ids);
      for (const [id, window] of this.#chooserWindows) {
        if (!active.has(id)) {
          this.#chooserWindows.delete(id);
          window.close();
        }
      }
      for (const id of ids) {
        if (this.#chooserWindows.has(id)) continue;
        const window = new this.#sdk.BrowserWindow({
          title: "Choose a file · BoxBox",
          url: chooserBootstrap(server.url, id),
          renderer: "cef",
          sandbox: true,
          allowedProtocols: { views: false, appData: false },
          frame: BROWSER_FRAME,
        });
        this.#chooserWindows.set(id, window);
        this.#track(window, () => {
          this.#chooserWindows.delete(id);
          void cancelChooser(server.url, id).catch(() => undefined);
        });
      }
    }).catch((error: unknown) => {
      if (!this.#quitting)
        console.error("File chooser connection failed:", error);
    });
    this.#instance?.setRequestHandler((next) => this.#handle(next));
  }

  async quit(code = 0): Promise<void> {
    if (this.#quitting) return;
    this.#quitting = true;
    this.#chooserAbort.abort();
    await this.#server.stop().catch((error) => {
      console.error("Could not stop the local BoxBox server:", error);
    });
    await this.#instance?.close().catch((error) => {
      console.error(
        "Could not close the BoxBox Desktop instance socket:",
        error,
      );
    });
    // Electrobun begins native/CEF shutdown synchronously from this call.
    this.#sdk.Utils.quit(code);
  }

  async #handle(request: InstanceRequest): Promise<void> {
    if (this.#quitting) return;
    switch (request.kind) {
      case "portal":
        return;
      case "activate":
        this.#focusPrimary();
        return;
      case "local":
        await this.#openLocal(request.target);
        return;
      case "remote":
        await this.#openRemote(request.url);
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
      if (target) {
        this.#pendingTargets.push(target);
        if (this.#localReady) this.#deliverTargets(existing, server.url);
      }
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
    window.webview.on("dom-ready", () => {
      window.webview.executeJavascript(`
        if (location.origin === ${JSON.stringify(new URL(server.url).origin)} &&
            document.querySelector('meta[name="boxbox-desktop"]')) {
          document.documentElement.dataset.hardwareAcceleration = '${hardwareAccelerationEnabled()}';
        }
      `);
      if (process.env.BOXBOX_DESKTOP_SMOKE === "1") {
        window.webview.executeJavascript(`
          if (!window.boxboxPaintProbe) {
            window.boxboxPaintProbe = true;
            const seen = new Set();
            const report = () => {
              const path = new URLSearchParams(location.search).get('path');
              const row = [...document.querySelectorAll('[data-file-path]')].find(row => row.dataset.filePath?.startsWith(path + '/'));
              if (!path || !row || seen.has(path) || row.getBoundingClientRect().height === 0) return;
              seen.add(path);
              requestAnimationFrame(() => requestAnimationFrame(() => {
                fetch('/api/v1/_desktop-painted/' + path);
              }));
            };
            new MutationObserver(report).observe(document, {subtree:true,childList:true,attributes:true});
            report();
          }
        `);
      }
      this.#localReady = true;
      this.#deliverTargets(window, server.url);
    });
    this.#track(window, () => {
      if (this.#localWindowId === window.id) {
        this.#localWindowId = undefined;
        this.#localReady = false;
        this.#pendingTargets = [];
      }
    });
    return window;
  }

  #deliverTargets(window: BrowserWindow, bootstrapURL: string): void {
    for (const target of this.#pendingTargets.splice(0)) {
      window.webview.executeJavascript(
        rendererTargetScript(bootstrapURL, target),
      );
    }
  }

  async #openRemote(value: string): Promise<BrowserWindow> {
    if (this.#quitting) throw new Error("BoxBox Desktop is shutting down");
    const server = await this.#server.get();
    const target = await remoteLaunchTarget(server.url, value);
    return this.#openLocal(target);
  }
}
