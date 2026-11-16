import { test } from "node:test";
import assert from "node:assert/strict";
import { agentPrompt, defaultChoices, installCommand, parseFolder, parsePort, parseUser } from "./installScript";

test("the command pipes the setup script into bash with the visitor's words", () => {
  assert.equal(
    installCommand(defaultChoices),
    "curl -fsSL https://boxbox.radhey.dev/install.sh | bash -s -- \\\n  --folder /home/you \\\n  --port 8080 \\\n  --user admin",
  );
  assert.match(installCommand({ folder: "/srv/media", port: 9000, user: "radhey" }), /--folder \/srv\/media \\\n  --port 9000 \\\n  --user radhey$/);
});

test("the agent prompt carries the project link and the latest image, and no settings", () => {
  assert.match(agentPrompt, /Project: https:\/\/github\.com\/jR4dh3y\/BoxBox\n/);
  assert.match(agentPrompt, /ghcr\.io\/jr4dh3y\/boxbox:latest/);
  assert.match(agentPrompt, /never print or log a password/);
  assert.doesNotMatch(agentPrompt, /\/home\/you|8080|admin/);
});

test("nothing that a shell would act on is accepted", () => {
  for (const folder of ["", "home/you", "/srv/$(reboot)", "/srv/my files", "/srv/a;b", "/srv/`id`", "/srv/../etc", "/a\nb"]) {
    assert.equal(parseFolder(folder), null, folder);
  }
  assert.equal(parseFolder(" /mnt/storage-1/media.d "), "/mnt/storage-1/media.d");

  for (const port of ["", "0", "65536", "80a", "8 0", "-1", "8080; reboot"]) assert.equal(parsePort(port), null, port);
  assert.equal(parsePort("443"), 443);

  for (const user of ["", "1admin", "ad min", "a$b", "a-b", "x".repeat(33)]) assert.equal(parseUser(user), null, user);
  assert.equal(parseUser("media_2"), "media_2");
});
