import { afterEach, describe, expect, test } from "bun:test";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createServer, type Server, type Socket } from "node:net";
import {
  lstat,
  mkdtemp,
  mkdir,
  readFile,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  acquireInstance,
  forwardToRunningInstance,
  parseInstanceRequest,
  tryAcquireLaunchGuard,
  type InstanceRequest,
} from "./instance";

const workerModule = new URL("./instance.ts", import.meta.url).href;
const workerScript = `
const moduleUrl = process.env.BOXBOX_INSTANCE_MODULE;
const directory = process.env.BOXBOX_INSTANCE_DIRECTORY;
if (!moduleUrl || !directory) throw new Error("Missing test worker configuration");
const { acquireInstance, tryAcquireLaunchGuard } = await import(moduleUrl);
if (process.env.BOXBOX_INSTANCE_MODE === "launch-guard") {
  const guard = await tryAcquireLaunchGuard(directory);
  if (!guard) {
    console.log("BUSY");
    process.exit(0);
  }
  console.log("GUARD");
  process.on("SIGTERM", () => {
    void guard.close().then(() => {
      console.log("CLOSED");
      process.exit(0);
    });
  });
} else {
  const { access } = await import("node:fs/promises");
  const barrier = process.env.BOXBOX_INSTANCE_BARRIER;
  console.log("READY");
  if (barrier) {
    while (true) {
      try { await access(barrier); break; }
      catch { await new Promise((resolve) => setTimeout(resolve, 5)); }
    }
  }
  const result = await acquireInstance(directory, { kind: "activate" });
  if (result.kind === "forwarded") {
    console.log("FORWARDED");
    process.exit(0);
  }
  const instance = result.instance;
  let requestCount = 0;
  instance.setRequestHandler((request) => {
    requestCount += 1;
    console.log("REQUEST " + requestCount + " " + JSON.stringify(request));
  });
  console.log("OWNER");
  process.on("SIGTERM", () => {
    void instance.close().then(() => {
      console.log("CLOSED");
      process.exit(0);
    }, (error) => {
      console.error(error);
      process.exit(2);
    });
  });
}
`;

interface Worker {
  readonly child: ChildProcessWithoutNullStreams;
  readonly output: () => string;
  readonly waitFor: (text: string) => Promise<void>;
  readonly exited: Promise<void>;
}

interface OutputWaiter {
  readonly text: string;
  readonly resolve: () => void;
  readonly reject: (error: Error) => void;
  readonly timer: ReturnType<typeof setTimeout>;
}

const roots: string[] = [];
const workers: Worker[] = [];

afterEach(async () => {
  await Promise.all(workers.splice(0).map(stopWorker));
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

async function temporaryRoot(label: string): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), `boxbox-instance-${label}-`));
  roots.push(root);
  return root;
}

function startWorker(
  directory: string,
  barrier?: string,
  mode?: string,
): Worker {
  const child = spawn(process.execPath, ["-e", workerScript], {
    env: {
      ...process.env,
      BOXBOX_INSTANCE_MODULE: workerModule,
      BOXBOX_INSTANCE_DIRECTORY: directory,
      ...(barrier ? { BOXBOX_INSTANCE_BARRIER: barrier } : {}),
      ...(mode ? { BOXBOX_INSTANCE_MODE: mode } : {}),
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let capturedOutput = "";
  let hasExited = false;
  const waiters = new Set<OutputWaiter>();
  const capture = (chunk: Buffer | string) => {
    capturedOutput += chunk.toString();
    for (const waiter of waiters) {
      if (!capturedOutput.includes(waiter.text)) continue;
      clearTimeout(waiter.timer);
      waiters.delete(waiter);
      waiter.resolve();
    }
  };
  child.stdout.on("data", capture);
  child.stderr.on("data", capture);
  const exited = new Promise<void>((resolve) => {
    child.once("close", () => {
      hasExited = true;
      resolve();
      for (const waiter of waiters) {
        clearTimeout(waiter.timer);
        waiter.reject(new Error(`Worker exited before output ${waiter.text}; output was: ${capturedOutput}`));
      }
      waiters.clear();
    });
  });
  const worker: Worker = {
    child,
    output: () => capturedOutput,
    waitFor(text) {
      if (capturedOutput.includes(text)) return Promise.resolve();
      if (hasExited) return Promise.reject(new Error(`Worker already exited: ${text}`));
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          waiters.delete(waiter);
          reject(new Error(`Timed out waiting for ${text}; output was: ${capturedOutput}`));
        }, 10_000);
        const waiter = { text, resolve, reject, timer };
        waiters.add(waiter);
      });
    },
    exited,
  };
  workers.push(worker);
  return worker;
}

async function stopWorker(worker: Worker): Promise<void> {
  if (worker.child.exitCode === null && worker.child.signalCode === null) {
    worker.child.kill("SIGKILL");
  }
  await worker.exited;
}

async function endOwner(owner: Worker): Promise<void> {
  owner.child.kill("SIGTERM");
  await owner.waitFor("CLOSED");
  await owner.exited;
}

function listenTestServer(server: Server, socketPath: string): Promise<void> {
  return new Promise((resolve, reject) => {
    server.once("listening", resolve);
    server.once("error", reject);
    server.listen(socketPath);
  });
}

function closeTestServer(server: Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => error ? reject(error) : resolve());
  });
}

function closeTestSockets(sockets: Set<Socket>): void {
  for (const socket of sockets) socket.destroy();
  sockets.clear();
}

describe("desktop single-instance handoff", () => {
  test("accepts only validated launch requests", () => {
    expect(parseInstanceRequest({ kind: "activate" })).toEqual({ kind: "activate" });
    expect(
      parseInstanceRequest({
        kind: "local",
        target: { path: "root/home/ana/Docs", select: "root/home/ana/Docs/a.txt" },
      }),
    ).toEqual({
      kind: "local",
      target: { path: "root/home/ana/Docs", select: "root/home/ana/Docs/a.txt" },
    });
    expect(parseInstanceRequest({ kind: "local", target: { path: "root/../etc" } })).toBeUndefined();
    expect(
      parseInstanceRequest({
        kind: "local",
        target: { path: "root/home/ana", select: "root/home/ana/Docs/a.txt" },
      }),
    ).toBeUndefined();
    expect(parseInstanceRequest({ kind: "remote", url: "https://user:pass@example.test" })).toBeUndefined();
    expect(parseInstanceRequest({ kind: "activate", command: "anything" })).toBeUndefined();
  });

  test("preflight forwards to an owner and returns false when its socket is absent", async () => {
    const directory = join(await temporaryRoot("forward"), "private");
    expect(await forwardToRunningInstance(directory, { kind: "activate" })).toBe(false);

    const result = await acquireInstance(directory, { kind: "activate" });
    if (result.kind !== "owner") throw new Error("Expected a test instance owner");
    try {
      const received = new Promise<unknown>((resolve) => {
        result.instance.setRequestHandler(resolve);
      });
      const request: InstanceRequest = {
        kind: "local",
        target: { path: "root/tmp", select: "root/tmp/report.txt" },
      };
      expect(await forwardToRunningInstance(directory, request)).toBe(true);
      expect(await received).toEqual(request);
    } finally {
      await result.instance.close();
    }
  });

  test("preflight throws for malformed responses and timeouts", async () => {
    const root = await temporaryRoot("bad-response");
    const directory = join(root, "private");
    await mkdir(directory, { mode: 0o700 });
    const socketPath = join(directory, "boxbox-desktop.sock");
    const malformedSockets = new Set<Socket>();
    const malformed = createServer((socket) => {
      malformedSockets.add(socket);
      socket.on("close", () => malformedSockets.delete(socket));
      socket.end("not-json\n");
    });
    await listenTestServer(malformed, socketPath);
    try {
      await expect(
        forwardToRunningInstance(directory, { kind: "activate" }),
      ).rejects.toThrow("rejected");
    } finally {
      closeTestSockets(malformedSockets);
      await closeTestServer(malformed);
    }

    const timeoutDirectory = join(await temporaryRoot("timeout"), "private");
    await mkdir(timeoutDirectory, { mode: 0o700 });
    const stalledSockets = new Set<Socket>();
    const stalled = createServer((socket) => {
      stalledSockets.add(socket);
      socket.on("close", () => stalledSockets.delete(socket));
    });
    await listenTestServer(stalled, join(timeoutDirectory, "boxbox-desktop.sock"));
    try {
      await expect(
        forwardToRunningInstance(timeoutDirectory, { kind: "activate" }),
      ).rejects.toThrow("Timed out");
    } finally {
      closeTestSockets(stalledSockets);
      await closeTestServer(stalled);
    }
  });

  test("native launch guard is exclusive and released when its process crashes", async () => {
    const directory = join(await temporaryRoot("launch-guard"), "private");
    const holder = startWorker(directory, undefined, "launch-guard");
    await holder.waitFor("GUARD");
    expect(await tryAcquireLaunchGuard(directory)).toBeUndefined();

    holder.child.kill("SIGKILL");
    await holder.exited;
    const recoveredGuard = await tryAcquireLaunchGuard(directory);
    if (!recoveredGuard) throw new Error("Kernel did not release the crashed launch guard");
    await recoveredGuard.close();
  });

  test("rejects symlink and public runtime parents without replacing their contents", async () => {
    const root = await temporaryRoot("unsafe-parent");
    const publicParent = join(root, "public");
    await mkdir(publicParent, { mode: 0o755 });
    const publicInstance = join(publicParent, "instance");
    await expect(
      acquireInstance(publicInstance, { kind: "activate" }),
    ).rejects.toThrow("runtime parent");

    const realParent = join(root, "private-parent");
    await mkdir(realParent, { mode: 0o700 });
    const symlinkParent = join(root, "linked-parent");
    await symlink(realParent, symlinkParent);
    await expect(
      acquireInstance(join(symlinkParent, "instance"), { kind: "activate" }),
    ).rejects.toThrow("runtime parent");
  });

  test("forwards a simultaneous clean launch to exactly one owner process", async () => {
    const directory = join(await temporaryRoot("clean"), "private");
    const barrier = join(await temporaryRoot("clean-barrier"), "go");
    const contenders = [startWorker(directory, barrier), startWorker(directory, barrier)];
    await Promise.all(contenders.map((worker) => worker.waitFor("READY")));
    await writeFile(barrier, "go");
    await Promise.all(contenders.map((worker) => worker.waitFor("OWNER").catch(() => worker.waitFor("FORWARDED"))));

    expect(contenders.filter((worker) => worker.output().includes("OWNER"))).toHaveLength(1);
    expect(contenders.filter((worker) => worker.output().includes("FORWARDED"))).toHaveLength(1);
    const owner = contenders.find((worker) => worker.output().includes("OWNER"));
    if (!owner) throw new Error("No instance owner started");
    await owner.waitFor('REQUEST 1 {"kind":"activate"}');
    await endOwner(owner);
  });

  test("serializes concurrent recovery after an owner crashes", async () => {
    const directory = join(await temporaryRoot("stale"), "private");
    const crashedOwner = startWorker(directory);
    await crashedOwner.waitFor("OWNER");
    const socketPath = join(directory, "boxbox-desktop.sock");
    expect((await lstat(socketPath)).isSocket()).toBe(true);

    crashedOwner.child.kill("SIGKILL");
    await crashedOwner.exited;
    expect((await lstat(socketPath)).isSocket()).toBe(true);
    expect(await forwardToRunningInstance(directory, { kind: "activate" })).toBe(false);

    const barrier = join(await temporaryRoot("stale-barrier"), "go");
    const contenders = Array.from({ length: 8 }, () => startWorker(directory, barrier));
    await Promise.all(contenders.map((worker) => worker.waitFor("READY")));
    await writeFile(barrier, "go");
    await Promise.all(contenders.map((worker) =>
      Promise.race([worker.waitFor("OWNER"), worker.waitFor("FORWARDED")])
    ));

    const owners = contenders.filter((worker) => worker.output().includes("OWNER"));
    const forwarded = contenders.filter((worker) => worker.output().includes("FORWARDED"));
    expect(owners).toHaveLength(1);
    expect(forwarded).toHaveLength(contenders.length - 1);
    expect((await lstat(socketPath)).isSocket()).toBe(true);
    const owner = owners[0];
    if (!owner) throw new Error("No instance owner survived stale recovery");
    await owner.waitFor(`REQUEST ${contenders.length - 1} {"kind":"activate"}`);
    await endOwner(owner);
    await expect(lstat(socketPath)).rejects.toThrow();
  });

  test("does not replace an unexpected file at the socket path", async () => {
    const directory = join(await temporaryRoot("unexpected-file"), "private");
    await mkdir(directory, { mode: 0o700 });
    const socketPath = join(directory, "boxbox-desktop.sock");
    await writeFile(socketPath, "keep");
    await expect(
      acquireInstance(directory, { kind: "activate" }),
    ).rejects.toThrow("not a user-owned socket");
    expect(await readFile(socketPath, "utf8")).toBe("keep");
  });
});
