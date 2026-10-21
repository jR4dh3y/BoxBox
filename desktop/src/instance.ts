import { createHash } from "node:crypto";
import { chmod, lstat, mkdir, unlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, posix, resolve } from "node:path";
import { createConnection, createServer, type Server, type Socket } from "node:net";
import type { LaunchTarget } from "./launch";
import { remoteURL } from "./target";

export type InstanceRequest =
  | { readonly kind: "activate" }
  | { readonly kind: "local"; readonly target: LaunchTarget }
  | { readonly kind: "remote"; readonly url: string };

export interface InstanceOwner {
  setRequestHandler(
    handler: (request: InstanceRequest) => void | Promise<void>,
  ): void;
  close(): Promise<void>;
}

export type InstanceResult =
  | { readonly kind: "owner"; readonly instance: InstanceOwner }
  | { readonly kind: "forwarded" };

interface RequestQueue {
  enqueue(request: InstanceRequest): void;
  setHandler(handler: (request: InstanceRequest) => void | Promise<void>): void;
}

const SOCKET_NAME = "boxbox-desktop.sock";
const MAX_REQUEST_BYTES = 8192;
const SOCKET_TIMEOUT_MS = 2000;
const MAX_STALE_RECOVERY_ATTEMPTS = 8;
const RECOVERY_GUARD_WAIT_MS = 3000;

/** Returns the per-user runtime directory used for the Linux handoff socket. */
export function instanceDirectory(): string {
  const uid = process.getuid?.();
  if (uid === undefined) {
    throw new Error("BoxBox Desktop single-instance mode requires a Unix user ID");
  }
  const runtimeDirectory = process.env.XDG_RUNTIME_DIR;
  const baseDirectory = runtimeDirectory && isAbsolute(runtimeDirectory)
    ? runtimeDirectory
    : tmpdir();
  return join(baseDirectory, `boxbox-desktop-${uid}`);
}

interface InstancePaths {
  readonly privateDirectory: string;
  readonly socketPath: string;
}

async function prepareInstancePaths(directory: string): Promise<InstancePaths> {
  const privateDirectory = resolve(directory);
  await preparePrivateDirectory(privateDirectory);
  const socketPath = join(privateDirectory, SOCKET_NAME);
  if (Buffer.byteLength(socketPath) >= 108) {
    throw new Error("BoxBox Desktop runtime path is too long for a Unix socket");
  }
  return { privateDirectory, socketPath };
}

/**
 * Binds the per-user socket or forwards this launch request to its owner.
 * The private directory is the authentication boundary; the wire protocol
 * accepts only validated local targets, remote URLs, and activation requests.
 */
export async function acquireInstance(
  directory: string,
  request: InstanceRequest,
): Promise<InstanceResult> {
  const validatedRequest = parseInstanceRequest(request);
  if (!validatedRequest) throw new Error("Invalid BoxBox Desktop launch request");

  const { socketPath } = await prepareInstancePaths(directory);
  const guardName = recoveryGuardName(socketPath);

  const requests = createRequestQueue();
  for (let attempt = 0; attempt < MAX_STALE_RECOVERY_ATTEMPTS; attempt += 1) {
    const server = createServer((socket) =>
      receiveRequest(socket, requests.enqueue)
    );
    let listening = false;
    let identity: Awaited<ReturnType<typeof lstat>> | undefined;
    try {
      await listen(server, socketPath);
      listening = true;
      identity = await lstat(socketPath);
      if (!identity.isSocket() || identity.uid !== process.getuid?.()) {
        throw new Error("BoxBox Desktop created an unexpected instance socket");
      }
      await chmod(socketPath, 0o600);
      return {
        kind: "owner",
        instance: createOwner(server, socketPath, guardName, identity, requests),
      };
    } catch (error) {
      if (listening) {
        await closeServer(server);
        if (identity) {
          const ownedIdentity = identity;
          await withRecoveryGuard(guardName, () =>
            unlinkOwnedSocket(socketPath, ownedIdentity)
          );
        }
      }
      if (!isAddressInUse(error)) throw error;
      const guard = await acquireRecoveryGuard(guardName);
      try {
        // Snapshot, probe, and unlink stay under one process-shared guard. The
        // inode check alone cannot stop two recoverers from unlinking in turn.
        const staleCandidate = await socketIdentity(socketPath);
        if (!staleCandidate) continue;
        const forwardResult = await forwardRequest(socketPath, validatedRequest);
        if (forwardResult === "accepted") return { kind: "forwarded" };
        if (forwardResult === "rejected") {
          throw new Error("The running BoxBox Desktop rejected the launch request");
        }
        await removeStaleSocket(socketPath, staleCandidate);
      } finally {
        await guard.close();
      }
    }
  }

  throw new Error("Could not acquire the BoxBox Desktop instance socket");
}

/** Forwards a validated launch request without starting the native SDK. */
export async function forwardToRunningInstance(
  directory: string,
  request: InstanceRequest,
): Promise<boolean> {
  const validatedRequest = parseInstanceRequest(request);
  if (!validatedRequest) throw new Error("Invalid BoxBox Desktop launch request");
  const { socketPath } = await prepareInstancePaths(directory);
  if (!(await socketIdentity(socketPath))) return false;

  const result = await forwardRequest(socketPath, validatedRequest);
  if (result === "accepted") return true;
  if (result === "unavailable") return false;
  throw new Error("The running BoxBox Desktop rejected the launch request");
}

/** Reserves this user's native launch slot until the returned guard is closed. */
export async function tryAcquireLaunchGuard(
  directory: string,
): Promise<{ close(): Promise<void> } | undefined> {
  const { privateDirectory } = await prepareInstancePaths(directory);
  return tryAcquireKernelGuard(launchGuardName(privateDirectory));
}

/** Parses and constrains data received from another process. */
export function parseInstanceRequest(value: unknown): InstanceRequest | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  if (!isRecord(value)) return undefined;
  const record = value;
  if (record.kind === "activate" && Object.keys(record).length === 1) {
    return { kind: "activate" };
  }
  if (record.kind === "local" && Object.keys(record).length === 2) {
    const target = parseLaunchTargetMessage(record.target);
    return target === undefined ? undefined : { kind: "local", target };
  }
  if (
    record.kind === "remote" &&
    Object.keys(record).length === 2 &&
    typeof record.url === "string"
  ) {
    try {
      return { kind: "remote", url: remoteURL(record.url) };
    } catch {
      return undefined;
    }
  }
  return undefined;
}

function parseLaunchTargetMessage(value: unknown): LaunchTarget | undefined {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return undefined;
  }
  if (!isRecord(value)) return undefined;
  const record = value;
  const keys = Object.keys(record);
  if (
    typeof record.path !== "string" ||
    !isVirtualPath(record.path) ||
    (keys.length !== 1 && keys.length !== 2)
  ) {
    return undefined;
  }
  if (keys.length === 1) return { path: record.path };
  if (typeof record.select !== "string" || !isVirtualPath(record.select)) {
    return undefined;
  }
  const parent = posix.dirname(record.select);
  if (parent !== record.path) return undefined;
  return { path: record.path, select: record.select };
}

function isVirtualPath(value: string): boolean {
  if (value === "root") return true;
  if (!value.startsWith("root/")) return false;
  return value
    .slice("root/".length)
    .split("/")
    .every((segment) => segment.length > 0 && segment !== "." && segment !== "..");
}

async function preparePrivateDirectory(directory: string): Promise<void> {
  const uid = process.getuid?.();
  if (uid === undefined) {
    throw new Error("BoxBox Desktop single-instance mode requires a Unix user ID");
  }
  const parentInfo = await lstat(dirname(directory));
  const parentMode = parentInfo.mode & 0o7777;
  const isPrivateUserDirectory = parentInfo.uid === uid &&
    (parentMode & 0o077) === 0;
  const isPrivateSystemTemp = parentInfo.uid === 0 &&
    (parentMode & 0o1000) !== 0 &&
    (parentMode & 0o007) === 0o007;
  if (
    !parentInfo.isDirectory() ||
    (!isPrivateUserDirectory && !isPrivateSystemTemp)
  ) {
    throw new Error("BoxBox Desktop runtime parent is not user-private or sticky temporary storage");
  }
  await mkdir(directory, { recursive: true, mode: 0o700 });
  const info = await lstat(directory);
  if (!info.isDirectory() || info.uid !== uid) {
    throw new Error("BoxBox Desktop runtime directory is not user-owned");
  }
  if ((info.mode & 0o077) !== 0) await chmod(directory, 0o700);
}

function listen(server: Server, socketPath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const onListening = () => {
      server.off("error", onError);
      resolve();
    };
    const onError = (error: Error) => {
      server.off("listening", onListening);
      reject(error);
    };
    server.once("listening", onListening);
    server.once("error", onError);
    server.listen(socketPath);
  });
}

function closeServer(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) reject(error);
      else resolve();
    });
  });
}

function recoveryGuardName(socketPath: string): string {
  return kernelGuardName("recovery", socketPath);
}

function launchGuardName(directory: string): string {
  return kernelGuardName("launch", directory);
}

function kernelGuardName(kind: "launch" | "recovery", identity: string): string {
  const uid = process.getuid?.();
  if (uid === undefined) {
    throw new Error("BoxBox Desktop single-instance mode requires a Unix user ID");
  }
  const identityHash = createHash("sha256").update(identity).digest("hex").slice(0, 24);
  return `\0boxbox-desktop-${kind}-${uid}-${identityHash}`;
}

interface KernelGuard {
  close(): Promise<void>;
}

function tryAcquireKernelGuard(name: string): Promise<KernelGuard | undefined> {
  const server = createServer();
  return new Promise((resolve, reject) => {
    const onListening = () => {
      server.off("error", onError);
      let closePromise: Promise<void> | undefined;
      resolve({
        close() {
          closePromise ??= closeServer(server);
          return closePromise;
        },
      });
    };
    const onError = (error: NodeJS.ErrnoException) => {
      server.off("listening", onListening);
      if (error.code === "EADDRINUSE") resolve(undefined);
      else reject(error);
    };
    server.once("listening", onListening);
    server.once("error", onError);
    server.listen(name);
  });
}

async function acquireRecoveryGuard(name: string): Promise<KernelGuard> {
  const deadline = Date.now() + RECOVERY_GUARD_WAIT_MS;
  while (Date.now() < deadline) {
    const guard = await tryAcquireKernelGuard(name);
    if (guard) return guard;
    await delay(20);
  }
  throw new Error("Timed out waiting for BoxBox Desktop socket recovery");
}

async function withRecoveryGuard<T>(
  name: string,
  operation: () => Promise<T>,
): Promise<T> {
  const guard = await acquireRecoveryGuard(name);
  try {
    return await operation();
  } finally {
    await guard.close();
  }
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

type IncomingRequest = (request: InstanceRequest) => void;

function receiveRequest(socket: Socket, enqueue: IncomingRequest): void {
  let data = "";
  let handled = false;
  socket.setEncoding("utf8");
  socket.setTimeout(SOCKET_TIMEOUT_MS, () => socket.destroy());
  socket.on("error", () => socket.destroy());
  socket.on("data", (chunk: string) => {
    if (handled) return;
    data += chunk;
    if (Buffer.byteLength(data) > MAX_REQUEST_BYTES) {
      respond(socket, false);
      handled = true;
      return;
    }
    const newline = data.indexOf("\n");
    if (newline < 0) return;
    if (newline !== data.length - 1) {
      respond(socket, false);
      handled = true;
      return;
    }
    let message: unknown;
    try {
      message = JSON.parse(data.slice(0, newline));
    } catch {
      respond(socket, false);
      handled = true;
      return;
    }
    const request = parseEnvelope(message);
    if (!request) {
      respond(socket, false);
      handled = true;
      return;
    }
    handled = true;
    enqueue(request);
    respond(socket, true);
  });
}

function parseEnvelope(value: unknown): InstanceRequest | undefined {
  if (!isRecord(value)) return undefined;
  const record = value;
  if (
    Object.keys(record).length !== 2 ||
    record.version !== 1 ||
    !("request" in record)
  ) {
    return undefined;
  }
  return parseInstanceRequest(record.request);
}

function respond(socket: Socket, ok: boolean): void {
  socket.end(`${JSON.stringify({ ok })}\n`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

type ForwardResult = "accepted" | "rejected" | "unavailable";

function forwardRequest(
  socketPath: string,
  request: InstanceRequest,
): Promise<ForwardResult> {
  return new Promise((resolve, reject) => {
    const socket = createConnection(socketPath);
    let response = "";
    let settled = false;
    const finish = (result: ForwardResult) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeout);
      socket.destroy();
      resolve(result);
    };
    const timeout = setTimeout(() => {
      if (settled) return;
      settled = true;
      socket.destroy();
      reject(new Error("Timed out contacting the BoxBox Desktop instance"));
    }, SOCKET_TIMEOUT_MS);
    socket.on("error", (error: NodeJS.ErrnoException) => {
      if (settled) return;
      if (error.code === "ECONNREFUSED" || error.code === "ENOENT") {
        finish("unavailable");
      } else {
        settled = true;
        clearTimeout(timeout);
        reject(error);
      }
    });
    socket.on("connect", () => {
      socket.write(`${JSON.stringify({ version: 1, request })}\n`);
    });
    socket.setEncoding("utf8");
    socket.on("data", (chunk: string) => {
      response += chunk;
      if (Buffer.byteLength(response) > 128) {
        finish("rejected");
        return;
      }
      const newline = response.indexOf("\n");
      if (newline < 0) return;
      try {
        const parsed: unknown = JSON.parse(response.slice(0, newline));
        if (
          typeof parsed === "object" &&
          parsed !== null &&
          !Array.isArray(parsed) &&
          "ok" in parsed &&
          parsed.ok === true
        ) {
          finish("accepted");
          return;
        }
      } catch {
        // A malformed response cannot establish that the existing owner accepted the request.
      }
      finish("rejected");
    });
  });
}

async function socketIdentity(
  socketPath: string,
): Promise<Awaited<ReturnType<typeof lstat>> | undefined> {
  let info;
  try {
    info = await lstat(socketPath);
  } catch (error) {
    if (isMissing(error)) return undefined;
    throw error;
  }
  if (!info.isSocket() || info.uid !== process.getuid?.()) {
    throw new Error("BoxBox Desktop instance path is not a user-owned socket");
  }
  return info;
}

async function removeStaleSocket(
  socketPath: string,
  staleCandidate: Awaited<ReturnType<typeof lstat>>,
): Promise<boolean> {
  let info;
  try {
    info = await lstat(socketPath);
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
  if (
    !info.isSocket() ||
    info.uid !== process.getuid?.() ||
    info.dev !== staleCandidate.dev ||
    info.ino !== staleCandidate.ino
  ) {
    return false;
  }
  try {
    await unlink(socketPath);
    return true;
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
}

function isAddressInUse(error: unknown): boolean {
  return typeof error === "object" && error !== null && "code" in error &&
    error.code === "EADDRINUSE";
}

function isMissing(error: unknown): boolean {
  return typeof error === "object" && error !== null && "code" in error &&
    error.code === "ENOENT";
}

function createOwner(
  server: Server,
  socketPath: string,
  guardName: string,
  identity: Awaited<ReturnType<typeof lstat>>,
  requests: RequestQueue,
): InstanceOwner {
  let closePromise: Promise<void> | undefined;

  return {
    setRequestHandler: requests.setHandler,
    close() {
      closePromise ??= new Promise<void>((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }
          void withRecoveryGuard(guardName, () =>
            unlinkOwnedSocket(socketPath, identity)
          ).then(resolve, reject);
        });
      });
      return closePromise;
    },
  };
}

function createRequestQueue(): RequestQueue {
  const pendingRequests: InstanceRequest[] = [];
  let handler:
    | ((request: InstanceRequest) => void | Promise<void>)
    | undefined;
  let draining = false;

  const drainRequests = async () => {
    const currentHandler = handler;
    if (currentHandler === undefined || draining) return;
    draining = true;
    try {
      while (pendingRequests.length > 0) {
        const request = pendingRequests.shift();
        if (request !== undefined) await currentHandler(request);
      }
    } catch (error) {
      console.error("Could not handle a BoxBox Desktop launch request:", error);
    } finally {
      draining = false;
      if (pendingRequests.length > 0) void drainRequests();
    }
  };

  return {
    enqueue(request) {
      pendingRequests.push(request);
      void drainRequests();
    },
    setHandler(nextHandler) {
      handler = nextHandler;
      void drainRequests();
    },
  };
}

async function unlinkOwnedSocket(
  socketPath: string,
  identity: Awaited<ReturnType<typeof lstat>>,
): Promise<void> {
  let current;
  try {
    current = await lstat(socketPath);
  } catch (error) {
    if (isMissing(error)) return;
    throw error;
  }
  if (
    current.isSocket() &&
    current.uid === identity.uid &&
    current.dev === identity.dev &&
    current.ino === identity.ino
  ) {
    await unlink(socketPath);
  }
}
