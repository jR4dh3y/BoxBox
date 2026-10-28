import { test, expect } from "bun:test";
import { runInNewContext } from "node:vm";
import { rendererTargetScript } from "./renderer-target";

test("handoffs queue literal targets without navigation and enforce local desktop origin", () => {
  const target = {
    path: 'root/tmp/quotes";globalThis.injected=true;name',
    select: 'root/tmp/quotes";globalThis.injected=true;name/file.txt',
  };
  const script = rendererTargetScript(
    "http://127.0.0.1:1234/_desktop?token=private",
    target,
  );
  const pending: unknown[] = [];
  const window = Object.assign(new EventTarget(), {
    boxboxDesktopTargets: pending,
  });
  let events = 0;
  window.addEventListener("boxbox-desktop-open", () => events++);
  const sandbox = {
    window,
    location: { origin: "http://127.0.0.1:1234" },
    document: { querySelector: () => ({}) },
    Event,
  };
  runInNewContext(script, sandbox);
  expect(window.boxboxDesktopTargets).toEqual([target]);
  expect(events).toBe(1);
  expect(Object.hasOwn(sandbox, "injected")).toBe(false);
  sandbox.location.origin = "https://other.example";
  runInNewContext(script, sandbox);
  expect(events).toBe(1);
  expect(script).not.toContain("private");
});
