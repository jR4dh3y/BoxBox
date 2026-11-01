/** Watches the private server; portal data can open only the fixed local chooser route. */
export async function watchChoosers(
  bootstrap: string,
  signal: AbortSignal,
  update: (ids: string[]) => void,
): Promise<void> {
  const url = new URL("/api/v1/desktop/chooser", bootstrap);
  const token = new URL(bootstrap).searchParams.get("token");
  if (!token || !/^[a-f0-9]{64}$/.test(token) || url.hostname !== "127.0.0.1") {
    throw new Error("Invalid chooser bootstrap");
  }
  let known = "";
  while (!signal.aborted) {
    url.searchParams.set("known", known);
    try {
      const response = await fetch(url, {
        headers: { Cookie: `boxbox_desktop=${token}` },
        signal,
      });
      if (!response.ok) throw new Error("File chooser connection failed");
      const value: unknown = await response.json();
      if (!Array.isArray(value)) throw new Error("Invalid chooser response");
      const ids = value.map((item: unknown) => {
        if (
          typeof item !== "object" ||
          item === null ||
          !("id" in item) ||
          typeof item.id !== "string" ||
          !/^[a-f0-9]{32}$/.test(item.id)
        ) {
          throw new Error("Invalid chooser request");
        }
        return item.id;
      });
      known = ids.sort().join(",");
      update(ids);
    } catch {
      if (signal.aborted) return;
      // A transient server failure must not strand later portal requests.
      await new Promise<void>((resolve) => {
        const done = () => {
          clearTimeout(timer);
          signal.removeEventListener("abort", done);
          resolve();
        };
        const timer = setTimeout(done, 500);
        signal.addEventListener("abort", done, { once: true });
      });
    }
  }
}

export function chooserBootstrap(bootstrap: string, id: string): string {
  if (!/^[a-f0-9]{32}$/.test(id)) throw new Error("Invalid chooser ID");
  const url = new URL(bootstrap);
  url.searchParams.set("next", `/chooser?id=${id}`);
  return url.href;
}

export async function cancelChooser(
  bootstrap: string,
  id: string,
): Promise<void> {
  const url = new URL(`/api/v1/desktop/chooser/${id}`, bootstrap);
  const token = new URL(bootstrap).searchParams.get("token");
  await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Cookie: `boxbox_desktop=${token}`,
    },
    body: JSON.stringify({ cancel: true }),
  });
}
