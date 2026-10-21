import type { ElectrobunConfig } from "electrobun";

export default {
  app: {
    name: "BoxBox",
    identifier: "io.boxbox.desktop",
    version: "0.1.6",
  },
  runtime: { exitOnLastWindowClosed: false },
  scripts: { postBuild: "scripts/prepare-launcher.ts" },
  build: {
    mainProcess: "bun",
    bun: { entrypoint: "src/index.ts" },
    views: {
      "connection-manager": { entrypoint: "src/connection-ui/index.ts" },
    },
    copy: {
      "resources/boxbox-server": "boxbox-server",
      "src/connection-ui/index.html": "views/connection-manager/index.html",
      "src/connection-ui/style.css": "views/connection-manager/style.css",
    },
    linux: { bundleCEF: true, defaultRenderer: "cef" },
    mac: { bundleCEF: true, defaultRenderer: "cef" },
    win: { bundleCEF: true, defaultRenderer: "cef" },
  },
} satisfies ElectrobunConfig;
