import { chmod, mkdir, readFile, rename, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { remoteURL } from "./target";

export interface ConnectionProfile {
  readonly id: string;
  readonly name: string;
  readonly url: string;
}

export type ConnectionSaveCommand =
  | { readonly kind: "create"; readonly name: string; readonly url: string }
  | {
      readonly kind: "update";
      readonly id: string;
      readonly name: string;
      readonly url: string;
    };

const CONNECTIONS_FILE = "connections.json";
const CONNECTION_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
const MAX_NAME_LENGTH = 80;
const MAX_URL_LENGTH = 2048;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOnlyKeys(
  value: Record<string, unknown>,
  keys: readonly string[],
): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
}

function parseConnectionId(value: unknown): string {
  if (typeof value !== "string" || !CONNECTION_ID.test(value)) {
    throw new Error("Connection ID is invalid");
  }
  return value.toLowerCase();
}

function parseName(value: unknown): string {
  if (typeof value !== "string") throw new Error("Enter a connection name");
  const name = value.trim();
  if (
    name.length === 0 ||
    name.length > MAX_NAME_LENGTH ||
    /[\u0000-\u001f\u007f]/u.test(name)
  ) {
    throw new Error("Use a name between 1 and 80 characters without control characters");
  }
  return name;
}

function parseURL(value: unknown): string {
  if (typeof value !== "string") throw new Error("Enter an HTTP(S) server URL");
  const input = value.trim();
  if (input.length === 0 || input.length > MAX_URL_LENGTH) {
    throw new Error("Enter an HTTP(S) server URL up to 2048 characters");
  }
  try {
    return remoteURL(input);
  } catch {
    throw new Error("Use an HTTP(S) BoxBox server URL without embedded credentials");
  }
}

function parseConnectionProfile(value: unknown): ConnectionProfile {
  if (
    !isRecord(value) ||
    !hasOnlyKeys(value, ["id", "name", "url"])
  ) {
    throw new Error("Saved connection data is invalid");
  }
  return {
    id: parseConnectionId(value["id"]),
    name: parseName(value["name"]),
    url: parseURL(value["url"]),
  };
}

function parseSaveCommand(value: unknown): ConnectionSaveCommand {
  if (!isRecord(value) || typeof value["kind"] !== "string") {
    throw new Error("Connection changes are invalid");
  }
  if (value["kind"] === "create") {
    if (!hasOnlyKeys(value, ["kind", "name", "url"])) {
      throw new Error("Connection changes are invalid");
    }
    return {
      kind: "create",
      name: parseName(value["name"]),
      url: parseURL(value["url"]),
    };
  }
  if (value["kind"] === "update") {
    if (!hasOnlyKeys(value, ["kind", "id", "name", "url"])) {
      throw new Error("Connection changes are invalid");
    }
    return {
      kind: "update",
      id: parseConnectionId(value["id"]),
      name: parseName(value["name"]),
      url: parseURL(value["url"]),
    };
  }
  throw new Error("Connection changes are invalid");
}

function parseStoredProfiles(value: unknown): ConnectionProfile[] {
  if (!Array.isArray(value)) throw new Error("Saved connections must be a list");
  const profiles = value.map(parseConnectionProfile);
  const ids = new Set<string>();
  const names = new Set<string>();
  for (const profile of profiles) {
    const normalizedName = profile.name.normalize("NFKC").toLowerCase();
    if (ids.has(profile.id) || names.has(normalizedName)) {
      throw new Error("Saved connections contain duplicate IDs or names");
    }
    ids.add(profile.id);
    names.add(normalizedName);
  }
  return profiles;
}

/** Saves named, credential-free HTTP(S) server URLs in a private user-data file. */
export class ConnectionsStore {
  private readonly directoryPath: string;
  private readonly filePath: string;
  private operationTail: Promise<void> = Promise.resolve();

  constructor(userDataPath: string) {
    this.directoryPath = userDataPath;
    this.filePath = join(userDataPath, CONNECTIONS_FILE);
  }

  list(): Promise<ConnectionProfile[]> {
    return this.serialize(async () => (await this.read()).map((profile) => ({ ...profile })));
  }

  async get(value: unknown): Promise<ConnectionProfile> {
    if (!isRecord(value) || !hasOnlyKeys(value, ["id"])) {
      throw new Error("Connection ID is invalid");
    }
    const id = parseConnectionId(value["id"]);
    const profile = (await this.list()).find((candidate) => candidate.id === id);
    if (!profile) throw new Error("Connection no longer exists");
    return profile;
  }

  async save(value: unknown): Promise<ConnectionProfile> {
    const command = parseSaveCommand(value);
    return this.serialize(async () => {
      const profiles = await this.read();
      const normalizedName = command.name.normalize("NFKC").toLowerCase();
      if (
        profiles.some(
          (profile) =>
            profile.id !== (command.kind === "update" ? command.id : undefined) &&
            profile.name.normalize("NFKC").toLowerCase() === normalizedName,
        )
      ) {
        throw new Error("A connection with that name already exists");
      }

      let saved: ConnectionProfile;
      if (command.kind === "create") {
        saved = {
          id: crypto.randomUUID(),
          name: command.name,
          url: command.url,
        };
        profiles.push(saved);
      } else {
        const index = profiles.findIndex((profile) => profile.id === command.id);
        if (index < 0) throw new Error("Connection no longer exists");
        saved = { id: command.id, name: command.name, url: command.url };
        profiles[index] = saved;
      }

      await this.write(profiles);
      return { ...saved };
    });
  }

  async remove(value: unknown): Promise<void> {
    if (!isRecord(value) || !hasOnlyKeys(value, ["id"])) {
      throw new Error("Connection ID is invalid");
    }
    const id = parseConnectionId(value["id"]);
    return this.serialize(async () => {
      const profiles = await this.read();
      const remaining = profiles.filter((profile) => profile.id !== id);
      if (remaining.length === profiles.length) throw new Error("Connection no longer exists");
      await this.write(remaining);
    });
  }

  private serialize<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.operationTail.then(operation);
    this.operationTail = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }

  private async read(): Promise<ConnectionProfile[]> {
    let contents: string;
    try {
      contents = await readFile(this.filePath, "utf8");
    } catch (error) {
      if (isRecord(error) && error["code"] === "ENOENT") return [];
      throw error;
    }
    let value: unknown;
    try {
      value = JSON.parse(contents);
    } catch {
      throw new Error("Saved connections could not be read because the file is invalid");
    }
    return parseStoredProfiles(value);
  }

  private async write(profiles: readonly ConnectionProfile[]): Promise<void> {
    await mkdir(this.directoryPath, { recursive: true, mode: 0o700 });
    const temporaryPath = `${this.filePath}.${crypto.randomUUID()}.tmp`;
    try {
      await writeFile(temporaryPath, `${JSON.stringify(profiles, null, 2)}\n`, {
        encoding: "utf8",
        mode: 0o600,
        flag: "wx",
      });
      await rename(temporaryPath, this.filePath);
      await chmod(this.filePath, 0o600);
    } catch (error) {
      await rm(temporaryPath, { force: true });
      throw error;
    }
  }
}
