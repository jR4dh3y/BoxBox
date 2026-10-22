import { Electroview } from "electrobun/view";
import type { ConnectionProfile, ConnectionSaveCommand } from "../connections";
import type { ConnectionsRPC } from "../connections-rpc";

type Editor =
  | { readonly kind: "create" }
  | { readonly kind: "update"; readonly profile: ConnectionProfile };

type Profiles =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly profiles: readonly ConnectionProfile[] }
  | { readonly kind: "failed"; readonly message: string };

/** The action in flight; `id` names the row it belongs to. */
type Busy = { readonly action: "save" | "connect" | "remove" | "local"; readonly id?: string };

const root = document.querySelector<HTMLElement>("#app");
if (!root) throw new Error("Connections view is missing its root element");
const app: HTMLElement = root;

const rpc = Electroview.defineRPC<ConnectionsRPC>({ handlers: { requests: {} } });
new Electroview({ rpc });

let profiles: Profiles = { kind: "loading" };
let editor: Editor | undefined;
/** What the user typed; survives re-renders while a save is in flight or after it fails. */
let draft = { name: "", url: "" };
let confirmRemoveId: string | undefined;
let busy: Busy | undefined;
let error = "";

function element<K extends keyof HTMLElementTagNameMap>(
  name: K,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const node = document.createElement(name);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function button(
  label: string,
  style: "primary" | "secondary" | "quiet" | "danger",
  onClick: () => void,
): HTMLButtonElement {
  const node = element("button", `button button-${style}`, label);
  node.type = "button";
  node.disabled = busy !== undefined;
  node.addEventListener("click", onClick);
  return node;
}

function running(action: Busy["action"], id?: string): boolean {
  return busy?.action === action && busy.id === id;
}

function describeError(value: unknown, fallback: string): string {
  return value instanceof Error && value.message.length > 0 ? value.message : fallback;
}

/** Runs one RPC at a time and shows its failure; success is visible in the list itself. */
async function run(
  next: Busy,
  failure: string,
  operation: () => Promise<void>,
): Promise<void> {
  if (busy) return;
  busy = next;
  error = "";
  render();
  try {
    await operation();
  } catch (caught) {
    error = describeError(caught, failure);
  }
  busy = undefined;
  render();
}

async function loadProfiles(): Promise<void> {
  try {
    profiles = { kind: "ready", profiles: await rpc.request.listConnections() };
  } catch (caught) {
    profiles = {
      kind: "failed",
      message: describeError(caught, "Could not load saved connections"),
    };
  }
}

function openEditor(next: Editor | undefined): void {
  editor = next;
  draft = next?.kind === "update"
    ? { name: next.profile.name, url: next.profile.url }
    : { name: "", url: "" };
  confirmRemoveId = undefined;
  error = "";
  render();
  app.querySelector<HTMLInputElement>("#connection-name")?.focus();
}

function render(): void {
  const header = element("header", "manager-header");
  header.append(
    element("h1", undefined, "Connections"),
    button(running("local") ? "Opening…" : "Local files", "secondary", () => {
      void run({ action: "local" }, "Could not open local files", () =>
        rpc.request.showLocalFiles(),
      );
    }),
    button("Add server", "primary", () => openEditor({ kind: "create" })),
  );

  const shell = element("section", "manager-shell");
  shell.append(header);
  if (error) {
    const alert = element("p", "status-message", error);
    alert.setAttribute("role", "alert");
    shell.append(alert);
  }
  if (editor) shell.append(renderEditor(editor));
  shell.append(renderProfiles());
  app.replaceChildren(shell);
}

function input(
  id: string,
  label: string,
  field: keyof typeof draft,
): [HTMLLabelElement, HTMLInputElement] {
  const labelNode = element("label", "field-label", label);
  labelNode.htmlFor = id;
  const node = element("input", "text-input");
  node.id = id;
  node.required = true;
  node.autocomplete = "off";
  node.spellcheck = false;
  node.value = draft[field];
  node.addEventListener("input", () => (draft[field] = node.value));
  return [labelNode, node];
}

function renderEditor(current: Editor): HTMLElement {
  const profile = current.kind === "update" ? current.profile : undefined;
  const [nameLabel, name] = input("connection-name", "Name", "name");
  name.maxLength = 80;
  name.placeholder = "Home server";
  const [urlLabel, url] = input("connection-url", "Server URL", "url");
  url.type = "url";
  url.maxLength = 2048;
  url.placeholder = "https://files.example.com";

  const save = element("button", "button button-primary", running("save") ? "Saving…" : "Save");
  save.type = "submit";
  save.disabled = busy !== undefined;

  const form = element("form", "connection-form");
  form.setAttribute("aria-label", profile ? `Edit ${profile.name}` : "Add server");
  form.append(
    nameLabel,
    name,
    urlLabel,
    url,
    save,
    button("Discard", "quiet", () => openEditor(undefined)),
  );
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const command: ConnectionSaveCommand = profile
      ? { kind: "update", id: profile.id, name: name.value, url: url.value }
      : { kind: "create", name: name.value, url: url.value };
    void run({ action: "save" }, "Could not save this connection", async () => {
      await rpc.request.saveConnection(command);
      editor = undefined;
      await loadProfiles();
    });
  });
  return form;
}

function renderProfiles(): HTMLElement {
  if (profiles.kind === "loading") {
    return element("p", "empty-state", "Loading saved connections…");
  }
  if (profiles.kind === "failed") {
    const failed = element("p", "status-message", profiles.message);
    failed.setAttribute("role", "alert");
    return failed;
  }
  if (profiles.profiles.length === 0) {
    return element("p", "empty-state", "No servers saved");
  }

  const list = element("ul", "connection-rows");
  list.setAttribute("aria-label", "Saved servers");
  for (const profile of profiles.profiles) {
    const row = element("li", "connection-row");
    row.append(
      element("span", "connection-name", profile.name),
      element("span", "connection-url", profile.url),
    );
    const actions = element("div", "connection-actions");
    if (confirmRemoveId === profile.id) {
      actions.append(
        button("Keep", "quiet", () => {
          confirmRemoveId = undefined;
          render();
        }),
        button(running("remove", profile.id) ? "Removing…" : "Remove", "danger", () => {
          void run({ action: "remove", id: profile.id }, `Could not remove ${profile.name}`, async () => {
            await rpc.request.removeConnection({ id: profile.id });
            confirmRemoveId = undefined;
            await loadProfiles();
          });
        }),
      );
    } else {
      actions.append(
        button(running("connect", profile.id) ? "Opening…" : "Connect", "primary", () => {
          void run({ action: "connect", id: profile.id }, `Could not open ${profile.name}`, () =>
            rpc.request.connectProfile({ id: profile.id }),
          );
        }),
        button("Edit", "quiet", () => openEditor({ kind: "update", profile })),
        button("Remove", "quiet", () => {
          confirmRemoveId = profile.id;
          editor = undefined;
          render();
        }),
      );
    }
    row.append(actions);
    list.append(row);
  }
  return list;
}

render();
void loadProfiles().then(render);
