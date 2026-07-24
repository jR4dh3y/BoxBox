#!/usr/bin/env bun
/**
 * Development script for running frontend and backend concurrently
 * with colored output prefixes.
 *
 * Usage: bun run dev
 */

import { spawn, type Subprocess } from "bun";

// ANSI color codes
const colors = {
  reset: "\x1b[0m",
  cyan: "\x1b[36m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  dim: "\x1b[2m",
  bold: "\x1b[1m",
};

const prefixes = {
  frontend: `${colors.cyan}[FE]${colors.reset}`,
  backend: `${colors.yellow}[BE]${colors.reset}`,
};

// Track child processes for cleanup
const processes: Subprocess[] = [];

/**
 * Prefix each line of output with a colored label
 */
function prefixStream(
  stream: ReadableStream<Uint8Array>,
  prefix: string
): void {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  const processChunk = async () => {
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          if (line.trim()) {
            console.log(`${prefix} ${line}`);
          }
        }
      }

      // Flush remaining buffer
      if (buffer.trim()) {
        console.log(`${prefix} ${buffer}`);
      }
    } catch {
      // Stream closed
    }
  };

  processChunk();
}

/**
 * Spawn a process with prefixed output
 */
function spawnWithPrefix(
  name: keyof typeof prefixes,
  cmd: string[],
  cwd?: string
): Subprocess {
  const prefix = prefixes[name];

  console.log(
    `${prefix} ${colors.dim}Starting: ${cmd.join(" ")}${colors.reset}`
  );

  const proc = spawn({
    cmd,
    cwd,
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      FORCE_COLOR: "1", // Enable colors in child processes
    },
  });

  if (proc.stdout) prefixStream(proc.stdout, prefix);
  if (proc.stderr) prefixStream(proc.stderr, `${prefix} ${colors.red}`);

  processes.push(proc);

  proc.exited.then((code) => {
    if (code !== 0 && code !== null) {
      console.log(
        `${prefix} ${colors.red}Process exited with code ${code}${colors.reset}`
      );
    }
  });

  return proc;
}

/**
 * Graceful shutdown handler
 */
async function shutdown(signal: string): Promise<void> {
  console.log(
    `\n${colors.dim}Received ${signal}, shutting down...${colors.reset}`
  );

  for (const proc of processes) {
    try {
      proc.kill("SIGTERM");
    } catch {
      // Process may already be dead
    }
  }

  // Wait a bit for graceful shutdown
  await Bun.sleep(500);

  // Force kill any remaining processes
  for (const proc of processes) {
    try {
      proc.kill("SIGKILL");
    } catch {
      // Process may already be dead
    }
  }

  process.exit(0);
}

// Register signal handlers
process.on("SIGINT", () => shutdown("SIGINT"));
process.on("SIGTERM", () => shutdown("SIGTERM"));

// Main entry point
async function main(): Promise<void> {
  console.log(`
${colors.bold}${colors.cyan}BoxBox - Development Mode${colors.reset}
${colors.dim}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${colors.reset}
`);

  // Check for air installation
  const airCheck = spawn({
    cmd: ["which", "air"],
    stdout: "pipe",
    stderr: "pipe",
  });
  await airCheck.exited;

  if (airCheck.exitCode !== 0) {
    console.log(`${colors.red}${colors.bold}Error:${colors.reset} 'air' is not installed.`);
    console.log(`${colors.dim}Install it with: go install github.com/air-verse/air@latest${colors.reset}`);
    console.log(`${colors.dim}Make sure $GOPATH/bin is in your PATH${colors.reset}\n`);
    process.exit(1);
  }

  // Start frontend (Vite dev server)
  spawnWithPrefix("frontend", ["bun", "run", "dev"], "./frontend");

  // Start backend (Go with air hot reload)
  spawnWithPrefix("backend", ["air", "-c", ".air.toml"], "./backend");

  // Keep the process alive
  await new Promise(() => {});
}

main().catch((err) => {
  console.error(`${colors.red}Fatal error:${colors.reset}`, err);
  process.exit(1);
});
