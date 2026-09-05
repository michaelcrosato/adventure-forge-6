import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, symlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { compareSnapshots, snapshotDist } from "./check-build.mjs";

function fixture(run) {
  const directory = mkdtempSync(join(tmpdir(), "forge-browser-admission-"));
  try { run(directory); } finally { rmSync(directory, { recursive: true, force: true }); }
}

test("identical paths and bytes pass regardless of insertion order", () => {
  const before = new Map([["index.html", Buffer.from("html")], ["assets/a.js", Buffer.from("js")]]);
  compareSnapshots(before, new Map([...before].reverse()));
});

test("changed bytes, renamed files, missing files, and extra files fail", () => {
  const before = new Map([["index.html", Buffer.from("html")], ["assets/a.js", Buffer.from("js")]]);
  for (const after of [
    new Map([["index.html", Buffer.from("changed")], ["assets/a.js", Buffer.from("js")]]),
    new Map([["index.html", Buffer.from("html")], ["assets/b.js", Buffer.from("js")]]),
    new Map([["index.html", Buffer.from("html")]]),
    new Map([...before, ["assets/stale.js", Buffer.from("old")]]),
  ]) assert.throws(() => compareSnapshots(before, after), /build admission failed/);
});

test("the snapshot includes every nested regular file, without consulting Git", () => fixture((root) => {
  mkdirSync(join(root, "assets", "nested"), { recursive: true });
  writeFileSync(join(root, "index.html"), "html");
  writeFileSync(join(root, "assets", "nested", "untracked.js"), "extra");
  assert.deepEqual([...snapshotDist(root).keys()], ["assets/nested/untracked.js", "index.html"]);
}));

test("missing or empty index fails closed", () => fixture((root) => {
  assert.throws(() => snapshotDist(root), /index.html is missing or empty/);
  writeFileSync(join(root, "index.html"), "");
  assert.throws(() => snapshotDist(root), /index.html is missing or empty/);
}));

test("symlink files, directories, and dist roots fail closed", () => fixture((root) => {
  const dist = join(root, "dist");
  mkdirSync(dist);
  writeFileSync(join(dist, "index.html"), "html");
  writeFileSync(join(root, "outside.js"), "outside");
  symlinkSync(join(root, "outside.js"), join(dist, "link.js"));
  assert.throws(() => snapshotDist(dist), /symlink/);
  rmSync(join(dist, "link.js"));
  symlinkSync(root, join(dist, "linked"));
  assert.throws(() => snapshotDist(dist), /symlink/);
  symlinkSync(dist, join(root, "dist-link"));
  assert.throws(() => snapshotDist(join(root, "dist-link")), /symlink/);
}));
