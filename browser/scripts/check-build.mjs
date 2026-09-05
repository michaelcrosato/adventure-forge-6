import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdirSync, lstatSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const browserDir = dirname(dirname(fileURLToPath(import.meta.url)));

function fail(message) {
  throw new Error(`browser build admission failed: ${message}`);
}

export function snapshotDist(distDir = join(browserDir, "dist")) {
  const relativeName = (path) => relative(distDir, path).split(sep).join("/");
  let root;
  try {
    root = lstatSync(distDir);
  } catch (error) {
    fail(`dist is unavailable: ${error.message}`);
  }
  if (root.isSymbolicLink()) {
    fail("dist may not be a symlink");
  }
  if (!root.isDirectory()) {
    fail("dist is not a directory");
  }

  const files = new Map();
  function visit(directory) {
    let entries;
    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch (error) {
      fail(`cannot read ${relativeName(directory)}: ${error.message}`);
    }
    entries.sort((left, right) => (left.name < right.name ? -1 : left.name > right.name ? 1 : 0));
    for (const entry of entries) {
      const path = join(directory, entry.name);
      let metadata;
      try {
        metadata = lstatSync(path);
      } catch (error) {
        fail(`cannot inspect ${relativeName(path)}: ${error.message}`);
      }
      if (metadata.isSymbolicLink()) {
        fail(`symlink is not admissible: ${relativeName(path)}`);
      }
      if (metadata.isDirectory()) {
        visit(path);
        continue;
      }
      if (!metadata.isFile()) {
        fail(`non-regular dist entry is not admissible: ${relativeName(path)}`);
      }
      let bytes;
      try {
        bytes = readFileSync(path);
      } catch (error) {
        fail(`cannot read ${relativeName(path)}: ${error.message}`);
      }
      files.set(relativeName(path), bytes);
    }
  }

  visit(distDir);
  if (!files.has("index.html") || files.get("index.html").length === 0) {
    fail("dist/index.html is missing or empty");
  }
  if (files.size === 0) {
    fail("dist contains no regular files");
  }
  return files;
}

function digest(snapshot) {
  const hash = createHash("sha256");
  for (const [path, bytes] of [...snapshot].sort(([left], [right]) =>
    left < right ? -1 : left > right ? 1 : 0,
  )) {
    hash.update(Buffer.from(path));
    hash.update(Buffer.from([0]));
    hash.update(bytes);
    hash.update(Buffer.from([0]));
  }
  return hash.digest("hex");
}

export function compareSnapshots(before, after) {
  const beforePaths = [...before.keys()].sort();
  const afterPaths = [...after.keys()].sort();
  if (beforePaths.length !== afterPaths.length) {
    fail(
      `build changed the asset count (before ${beforePaths.length}, after ${afterPaths.length})`,
    );
  }
  for (let index = 0; index < beforePaths.length; index += 1) {
    const beforePath = beforePaths[index];
    const afterPath = afterPaths[index];
    if (beforePath !== afterPath) {
      fail(`build changed the asset paths (${beforePath} versus ${afterPath})`);
    }
    const beforeBytes = before.get(beforePath);
    const afterBytes = after.get(afterPath);
    if (!beforeBytes.equals(afterBytes)) {
      fail(`build changed asset bytes: ${beforePath}`);
    }
  }
}

function runBuild() {
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = spawnSync(npm, ["run", "build"], {
    cwd: browserDir,
    env: { ...process.env },
    stdio: "inherit",
  });
  if (result.error) {
    fail(`could not run npm run build: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`npm run build exited with status ${result.status ?? "unknown"}`);
  }
}

function main() {
 try {
  const before = snapshotDist();
  const beforeDigest = digest(before);
  runBuild();
  const after = snapshotDist();
  compareSnapshots(before, after);
  const afterDigest = digest(after);
  if (beforeDigest !== afterDigest) {
    fail("build output digest changed despite matching paths and bytes");
  }
  process.stdout.write(`browser build is reproducible (${after.size} assets, ${afterDigest})\n`);
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
}
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
