import { readFileSync } from "node:fs";
import type { ElectrobunConfig } from "electrobun";

const version = readFileSync(
  new URL("../VERSION", import.meta.url),
  "utf8",
).trim();
if (!/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/.test(version))
  throw new Error("Invalid release VERSION");

export default {
  app: {
    name: "BoxBox",
    identifier: "io.boxbox.desktop",
    version,
  },
  runtime: { exitOnLastWindowClosed: false },
  scripts: { postBuild: "scripts/prepare-launcher.ts" },
  build: {
    mainProcess: "bun",
    bun: { entrypoint: "src/index.ts" },
    copy: {
      "resources/boxbox-server": "boxbox-server",
      "resources/native": "native",
    },
    linux: {
      bundleCEF: true,
      defaultRenderer: "cef",
      // Electrobun defaults to software rendering for VM compatibility.
      chromiumFlags: {
        "disable-gpu": false,
        "disable-gpu-compositing": false,
      },
    },
    mac: { bundleCEF: true, defaultRenderer: "cef" },
    win: { bundleCEF: true, defaultRenderer: "cef" },
  },
} satisfies ElectrobunConfig;
