import { test } from "node:test";
import assert from "node:assert/strict";
import { filesUnder, find, folderAt, formatBytes, placeFile, remove, search, typeOf, uniqueName, type Folder, type TextFile } from "./fs";
import { back, canGoForward, currentPath, forward, visit } from "./history";
import { tokenizeYamlLine } from "./highlight";
import { advance, progressOf, type Upload } from "./upload";

const note = (name: string, content = "x"): TextFile => ({ kind: "text", name, content });

function tree(): Folder {
  return {
    kind: "folder",
    name: "",
    children: [{ kind: "folder", name: "home", children: [{ kind: "folder", name: "docs", children: [note("a.txt")] }, note("b.txt")] }],
  };
}

test("an uploaded file keeps its relative path", () => {
  const root = tree();
  placeFile(root, "home", "trip/2026/c.txt", note("c.txt"));
  assert.equal(find(root, "home/trip/2026/c.txt")?.name, "c.txt");
  assert.deepEqual(filesUnder(root).map((file) => file.path), ["home/docs/a.txt", "home/trip/2026/c.txt", "home/b.txt"]);
});

test("a second copy gets a new name and removal takes one entry", () => {
  const root = tree();
  const home = folderAt(root, "home");
  assert.ok(home);
  assert.equal(uniqueName(home, "b.txt"), "b (2).txt");
  assert.equal(uniqueName(home, "docs"), "docs (2)");
  remove(root, "home/docs");
  assert.equal(find(root, "home/docs"), undefined);
  assert.equal(find(root, "home/b.txt")?.name, "b.txt");
});

test("search finds files and folders but not the mounts themselves", () => {
  assert.deepEqual(search(tree(), "O").map((match) => match.path), ["home/docs"]);
  assert.deepEqual(search(tree(), "  "), []);
});

test("sizes read like the app's", () => {
  assert.equal(formatBytes(342), "342 B");
  assert.equal(formatBytes(46_633), "45.54 KB");
});

test("visiting from the middle of history drops what was ahead", () => {
  let history = visit(visit({ entries: [""], index: 0 }, "home"), "home/docs");
  history = back(history);
  assert.equal(currentPath(history), "home");
  assert.equal(currentPath(forward(history)), "home/docs");
  history = visit(history, "home/trip");
  assert.equal(canGoForward(history), false);
  assert.deepEqual(history.entries, ["", "home", "home/trip"]);
});

test("an upload reports each file once, as it completes", () => {
  let upload: Upload = { label: "trip", target: "home", sent: 0, items: [{ relPath: "trip/a.txt", file: note("a.txt", "12345") }, { relPath: "trip/b.txt", file: note("b.txt", "12345") }] };
  const landed: string[] = [];
  for (let step = 0; step < 4; step += 1) {
    const result = advance(upload, 3);
    upload = result.upload;
    landed.push(...result.finished.map((item) => item.relPath));
  }
  assert.deepEqual(landed, ["trip/a.txt", "trip/b.txt"]);
  assert.deepEqual({ ...progressOf(upload), current: undefined }, { current: undefined, currentSent: 5, currentBytes: 5, completed: 2, percent: 100, done: true });
});

test("yaml lines split into keys and values", () => {
  assert.deepEqual(tokenizeYamlLine("  image: a:b").map((token) => token.kind), ["plain", "key", "plain", "value"]);
  assert.deepEqual(tokenizeYamlLine('      - "8080:8080"'), [{ text: "      - ", kind: "plain" }, { text: '"8080:8080"', kind: "value" }]);
  assert.deepEqual(tokenizeYamlLine("# note"), [{ text: "# note", kind: "comment" }]);
});

test("the Type column names files the way the app does", () => {
  assert.equal(typeOf({ kind: "folder", name: "Pictures", children: [] }), "Folder");
  assert.equal(typeOf(note("docker-compose.yml")), "YAML");
  assert.equal(typeOf(note("playlist.m3u")), "M3U File");
  assert.equal(typeOf(note("fstab")), "File");
});
