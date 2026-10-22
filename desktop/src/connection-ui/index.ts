import { Electroview } from "electrobun/view";
import type { ConnectionProfile, ConnectionSaveCommand } from "../connections";
import type { ConnectionsRPC } from "../connections-rpc";

type EditorState =
  | { readonly kind: "create" }
  | { readonly kind: "update"; readonly profile: ConnectionProfile };

type BusyState =
  | { readonly kind: "save" }
  | { readonly kind: "connect"; readonly id: string }
  | { readonly kind: "remove"; readonly id: string }
  | { readonly kind: "local" };

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("Connections view is missing its root element");

const rpc = Electroview.defineRPC<ConnectionsRPC>({
  handlers: { requests: {} },
});
new Electroview({ rpc });

let profiles: ConnectionProfile[] = [];
let editor: EditorState | undefined;
let confirmRemoveId: string | undefined;
let busy: BusyState | undefined;
let loading = true;
let statusMessage = "";
let statusKind: "error" | "success" | "info" = "info";

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
  node.addEventListener("click", onClick);
  return node;
}

function setStatus(message: string, kind: typeof statusKind = "info"): void {
  statusMessage = message;
  statusKind = kind;
  const node = app?.querySelector<HTMLElement>("[data-status]");
  if (!node) return;
  node.textContent = message;
  node.dataset.kind = kind;
  node.hidden = message.length === 0;
}

function isBusy(kind: BusyState["kind"], id?: string): boolean {
  if (!busy || busy.kind !== kind) return false;
  if (id === undefined) return true;
  return "id" in busy && busy.id === id;
}

function render(): void {
  if (!app) return;
  const shell = element("section", "manager-shell");
  const header = element("header", "manager-header");
  const titleBlock = element("div", "title-block");
  titleBlock.append(element("h1", undefined, "Connections"));
  const headerActions = element("div", "header-actions");
  const localButton = button("Local files", "secondary", () => {
    void showLocalFiles();
  });
  localButton.disabled = isBusy("local");
  headerActions.append(
    localButton,
    button("Add server", "primary", () => {
      editor = { kind: "create" };
      confirmRemoveId = undefined;
      render();
      app?.querySelector<HTMLInputElement>("#connection-name")?.focus();
    }),
  );
  header.append(titleBlock, headerActions);
  shell.append(header);

  const status = element("p", "status-message");
  status.dataset.status = "";
  status.dataset.kind = statusKind;
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");
  status.textContent = statusMessage;
  status.hidden = statusMessage.length === 0;
  shell.append(status);

  if (editor) shell.append(renderEditor(editor));
  shell.append(renderConnectionList());
  app.replaceChildren(shell);
}

function renderEditor(current: EditorState): HTMLElement {
  const panel = element("section", "editor-panel");
  panel.setAttribute("aria-labelledby", "editor-heading");
  const heading = element(
    "h2",
    undefined,
    current.kind === "create" ? "Add a server" : "Edit server",
  );
  heading.id = "editor-heading";
  panel.append(heading);

  const form = element("form", "connection-form");
  const nameLabel = element("label", "field-label", "Connection name");
  nameLabel.htmlFor = "connection-name";
  const nameInput = element("input", "text-input");
  nameInput.id = "connection-name";
  nameInput.name = "name";
  nameInput.type = "text";
  nameInput.required = true;
  nameInput.maxLength = 80;
  nameInput.autocomplete = "off";
  nameInput.spellcheck = false;
  nameInput.placeholder = "Home server";
  nameInput.value = current.kind === "update" ? current.profile.name : "";

  const urlLabel = element("label", "field-label", "Server URL");
  urlLabel.htmlFor = "connection-url";
  const urlInput = element("input", "text-input");
  urlInput.id = "connection-url";
  urlInput.name = "url";
  urlInput.type = "url";
  urlInput.required = true;
  urlInput.maxLength = 2048;
  urlInput.setAttribute("autocomplete", "url");
  urlInput.spellcheck = false;
  urlInput.placeholder = "https://files.example.com";
  urlInput.value = current.kind === "update" ? current.profile.url : "";

  const help = element(
    "p",
    "field-help",
    "Use an HTTP or HTTPS URL. Add sign-in details on the server itself; BoxBox does not save passwords.",
  );
  const fields = element("div", "form-fields");
  const nameField = element("div", "field");
  nameField.append(nameLabel, nameInput);
  const urlField = element("div", "field");
  urlField.append(urlLabel, urlInput, help);
  fields.append(nameField, urlField);

  const actions = element("div", "form-actions");
  const saveButton = element(
    "button",
    "button button-primary",
    isBusy("save") ? "Saving…" : "Save connection",
  );
  saveButton.type = "submit";
  saveButton.disabled = isBusy("save");
  actions.append(
    saveButton,
    button("Cancel", "quiet", () => {
      editor = undefined;
      render();
    }),
  );
  form.append(fields, actions);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const command: ConnectionSaveCommand = current.kind === "create"
      ? { kind: "create", name: nameInput.value, url: urlInput.value }
      : {
          kind: "update",
          id: current.profile.id,
          name: nameInput.value,
          url: urlInput.value,
        };
    void saveConnection(command, saveButton);
  });
  panel.append(form);
  return panel;
}

function renderConnectionList(): HTMLElement {
  const section = element("section", "connection-list");
  const listHeader = element("div", "list-heading");
  listHeader.append(
    element("h2", undefined, "Saved servers"),
    element("span", "connection-count", `${profiles.length}`),
  );
  section.append(listHeader);

  if (loading) {
    const loadingRow = element("div", "loading-row", "Loading saved connections…");
    loadingRow.setAttribute("aria-live", "polite");
    section.append(loadingRow);
    return section;
  }
  if (profiles.length === 0) {
    section.append(element("p", "empty-state", "No servers saved"));
    return section;
  }

  const list = element("ul", "connection-rows");
  for (const profile of profiles) {
    const row = element("li", "connection-row");
    const details = element("div", "connection-details");
    details.append(
      element("span", "connection-dot", ""),
      element("div", "connection-copy"),
    );
    const copy = details.querySelector<HTMLElement>(".connection-copy");
    copy?.append(
      element("strong", "connection-name", profile.name),
      element("span", "connection-url", profile.url),
    );

    const actions = element("div", "connection-actions");
    if (confirmRemoveId === profile.id) {
      const prompt = element("span", "remove-prompt", `Remove ${profile.name}?`);
      const confirm = button(
        isBusy("remove", profile.id) ? "Removing…" : "Remove",
        "danger",
        () => void removeConnection(profile),
      );
      confirm.disabled = isBusy("remove", profile.id);
      actions.append(
        prompt,
        button("Keep", "quiet", () => {
          confirmRemoveId = undefined;
          render();
        }),
        confirm,
      );
    } else {
      const connect = button(
        isBusy("connect", profile.id) ? "Opening…" : "Connect",
        "primary",
        () => void connectProfile(profile),
      );
      connect.disabled = busy !== undefined;
      const edit = button("Edit", "quiet", () => {
        editor = { kind: "update", profile };
        confirmRemoveId = undefined;
        render();
        app?.querySelector<HTMLInputElement>("#connection-name")?.focus();
      });
      edit.disabled = busy !== undefined;
      const remove = button("Remove", "quiet", () => {
        confirmRemoveId = profile.id;
        editor = undefined;
        render();
      });
      remove.classList.add("danger-text");
      remove.disabled = busy !== undefined;
      actions.append(connect, edit, remove);
    }

    row.append(details, actions);
    list.append(row);
  }
  section.append(list);
  return section;
}

async function refreshConnections(message = ""): Promise<void> {
  loading = true;
  render();
  try {
    profiles = await rpc.request.listConnections();
    loading = false;
    statusMessage = message;
    statusKind = "success";
    render();
  } catch (error) {
    loading = false;
    statusMessage = describeError(error, "Could not load saved connections");
    statusKind = "error";
    render();
  }
}

async function saveConnection(
  command: ConnectionSaveCommand,
  submitButton: HTMLButtonElement,
): Promise<void> {
  busy = { kind: "save" };
  submitButton.disabled = true;
  submitButton.textContent = "Saving…";
  setStatus("Saving connection…");
  try {
    const profile = await rpc.request.saveConnection(command);
    editor = undefined;
    confirmRemoveId = undefined;
    busy = undefined;
    await refreshConnections(`${profile.name} saved.`);
  } catch (error) {
    busy = undefined;
    submitButton.disabled = false;
    submitButton.textContent = "Save connection";
    setStatus(describeError(error, "Could not save this connection"), "error");
  }
}

async function connectProfile(profile: ConnectionProfile): Promise<void> {
  busy = { kind: "connect", id: profile.id };
  render();
  setStatus(`Opening ${profile.name}…`);
  try {
    await rpc.request.connectProfile({ id: profile.id });
    statusMessage = `${profile.name} opened in a new window.`;
    statusKind = "success";
  } catch (error) {
    statusMessage = describeError(error, `Could not open ${profile.name}`);
    statusKind = "error";
  }
  busy = undefined;
  render();
}

async function removeConnection(profile: ConnectionProfile): Promise<void> {
  busy = { kind: "remove", id: profile.id };
  render();
  try {
    await rpc.request.removeConnection({ id: profile.id });
    confirmRemoveId = undefined;
    busy = undefined;
    await refreshConnections(`${profile.name} removed.`);
  } catch (error) {
    busy = undefined;
    statusMessage = describeError(error, `Could not remove ${profile.name}`);
    statusKind = "error";
    render();
  }
}

async function showLocalFiles(): Promise<void> {
  busy = { kind: "local" };
  render();
  setStatus("Opening local files…");
  try {
    await rpc.request.showLocalFiles();
    busy = undefined;
    statusMessage = "Local files opened in a new window.";
    statusKind = "success";
  } catch (error) {
    busy = undefined;
    statusMessage = describeError(error, "Could not open local files");
    statusKind = "error";
  }
  render();
}

function describeError(error: unknown, fallback: string): string {
  return error instanceof Error && error.message.length > 0 ? error.message : fallback;
}

render();
void refreshConnections();
