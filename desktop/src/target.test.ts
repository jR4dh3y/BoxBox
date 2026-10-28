import { describe, expect, test } from "bun:test";
import { remoteURL, serverURL } from "./target";

describe("desktop targets", () => {
  test("remote servers retain their URL", () => {
    expect(remoteURL("https://files.example.test/browse")).toBe(
      "https://files.example.test/browse",
    );
    for (const value of [
      "file:///etc/passwd",
      "http://files.example.test",
      "https://files.example.test?token=secret",
      "https://files.example.test#fragment",
      "javascript:alert(1)",
      "https://user:password@example.test",
    ]) {
      expect(() => remoteURL(value)).toThrow();
    }
  });
  test("readiness accepts only an explicit loopback origin", () => {
    expect(serverURL('{"url":"http://127.0.0.1:12345"}')).toBe(
      "http://127.0.0.1:12345",
    );
    for (const url of [
      "http://example.test:12345",
      "http://127.0.0.1",
      "http://127.0.0.1:12345/?token=x",
    ]) {
      expect(() => serverURL(JSON.stringify({ url }))).toThrow();
    }
    for (const line of ["null", "{}", '{"url":12}', "invalid"])
      expect(() => serverURL(line)).toThrow();
  });
});
