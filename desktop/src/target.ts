export function remoteURL(value: string): string {
  const url = new URL(value);
  if (
    !["http:", "https:"].includes(url.protocol) ||
    url.username ||
    url.password
  ) {
    throw new Error(
      "Use an HTTP(S) BoxBox server URL without embedded credentials",
    );
  }
  return url.href;
}

export function serverURL(line: string): string {
  const value: unknown = JSON.parse(line);
  if (
    typeof value !== "object" ||
    value === null ||
    !("url" in value) ||
    typeof value.url !== "string"
  ) {
    throw new Error("Invalid desktop server readiness message");
  }
  const url = new URL(value.url);
  if (
    url.protocol !== "http:" ||
    url.hostname !== "127.0.0.1" ||
    !url.port ||
    url.pathname !== "/" ||
    url.search ||
    url.hash ||
    url.username ||
    url.password
  ) {
    throw new Error("Desktop server must listen on loopback");
  }
  return url.origin;
}
