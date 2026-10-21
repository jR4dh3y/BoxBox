import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { ConnectionsStore } from "./connections";

let userDataPath: string;

beforeEach(async () => {
  userDataPath = await mkdtemp(join(tmpdir(), "boxbox-connections-test-"));
});

afterEach(async () => {
  await rm(userDataPath, { recursive: true, force: true });
});

describe("saved BoxBox connections", () => {
  test("persists only credential-free profile fields with private file permissions", async () => {
    const store = new ConnectionsStore(userDataPath);
    const profile = await store.save({
      kind: "create",
      name: "  Home server  ",
      url: "https://files.example.test/",
    });
    const filePath = join(userDataPath, "connections.json");
    const persisted: unknown = JSON.parse(await readFile(filePath, "utf8"));
    const fileInfo = await stat(filePath);
    if (!Array.isArray(persisted)) throw new Error("Saved connections are not a list");
    const firstProfile: unknown = persisted[0];
    if (
      typeof firstProfile !== "object" ||
      firstProfile === null ||
      Array.isArray(firstProfile)
    ) {
      throw new Error("Saved profile is not an object");
    }

    expect(profile.name).toBe("Home server");
    expect(profile.url).toBe("https://files.example.test/");
    expect(persisted).toEqual([{ id: profile.id, name: profile.name, url: profile.url }]);
    expect(Object.keys(firstProfile).sort()).toEqual(["id", "name", "url"]);
    expect(fileInfo.mode & 0o777).toBe(0o600);
    expect(await new ConnectionsStore(userDataPath).list()).toEqual([profile]);
  });

  test("creates, updates, and removes a stable profile", async () => {
    const store = new ConnectionsStore(userDataPath);
    const created = await store.save({
      kind: "create",
      name: "Office",
      url: "http://boxbox.office.test",
    });
    const updated = await store.save({
      kind: "update",
      id: created.id,
      name: "Work files",
      url: "https://boxbox.office.test/",
    });

    expect(updated.id).toBe(created.id);
    expect(updated.name).toBe("Work files");
    expect(await store.list()).toEqual([updated]);
    await store.remove({ id: updated.id });
    expect(await store.list()).toEqual([]);
  });

  test("serializes concurrent saves without dropping a profile", async () => {
    const store = new ConnectionsStore(userDataPath);
    await Promise.all([
      store.save({ kind: "create", name: "One", url: "https://one.example.test" }),
      store.save({ kind: "create", name: "Two", url: "https://two.example.test" }),
    ]);

    expect((await store.list()).map((profile) => profile.name).sort()).toEqual([
      "One",
      "Two",
    ]);
  });

  test("rejects duplicate names, invalid fields, and embedded URL credentials", async () => {
    const store = new ConnectionsStore(userDataPath);
    await store.save({ kind: "create", name: "Home", url: "https://home.example.test" });

    const invalidProfiles: unknown[] = [
      { kind: "create", name: " home ", url: "https://other.example.test" },
      { kind: "create", name: "Bad\nName", url: "https://other.example.test" },
      { kind: "create", name: "Local", url: "file:///etc/passwd" },
      { kind: "create", name: "Script", url: "javascript:alert(1)" },
      { kind: "create", name: "Password", url: "https://user:secret@example.test" },
      { kind: "create", name: "Username", url: "https://user@example.test" },
      { kind: "create", name: "Unexpected", url: "https://example.test", token: "secret" },
    ];
    for (const profile of invalidProfiles) {
      await expect(store.save(profile)).rejects.toThrow();
    }
    await expect(store.remove({ id: "../connections.json" })).rejects.toThrow();
    expect((await store.list()).map((profile) => profile.name)).toEqual(["Home"]);
  });

  test("rejects corrupted stored data instead of silently resetting it", async () => {
    const filePath = join(userDataPath, "connections.json");
    await writeFile(filePath, "{not json", { mode: 0o600 });

    await expect(new ConnectionsStore(userDataPath).list()).rejects.toThrow(
      "Saved connections could not be read",
    );
  });
});
