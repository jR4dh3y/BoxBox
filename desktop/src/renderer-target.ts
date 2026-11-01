import type { LaunchTarget } from "./launch";

/** Queue launches before the workspace mounts; never navigate away from its current tabs. */
export function rendererTargetScript(
  bootstrapURL: string,
  target: LaunchTarget,
): string {
  const origin = new URL(bootstrapURL).origin;
  return `(() => {
    if (location.origin !== ${JSON.stringify(origin)} || !document.querySelector('meta[name="boxbox-desktop"]')) return;
    if (!Array.isArray(window.boxboxDesktopTargets)) window.boxboxDesktopTargets = [];
    window.boxboxDesktopTargets.push(${JSON.stringify(target)});
    window.dispatchEvent(new Event('boxbox-desktop-open'));
  })();`;
}
