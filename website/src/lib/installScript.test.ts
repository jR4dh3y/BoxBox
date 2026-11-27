import { test } from "node:test";
import assert from "node:assert/strict";
import { parseFolder, parsePort, parseUser } from "./installScript";

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
