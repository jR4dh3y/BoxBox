/** Quotes one argument for a Desktop Entry Exec line. */
export function quoteDesktopExecArgument(value: string): string {
  if (value.length === 0) {
    throw new Error("Desktop entry arguments cannot be empty");
  }
  if (/[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error(
      "Desktop entry arguments cannot contain control characters",
    );
  }
  const escaped = value.replaceAll("%", "%%").replace(/[\\"`$]/g, "\\$&");
  return `"${escaped}"`;
}
