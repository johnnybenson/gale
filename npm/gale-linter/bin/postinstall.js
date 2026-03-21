#!/usr/bin/env node

"use strict";

const { execSync, spawnSync } = require("child_process");
const os = require("os");

/**
 * Detect whether the current Linux system uses musl libc.
 */
function isMusl() {
  let stderr;
  try {
    stderr = execSync("ldd --version", {
      stdio: ["pipe", "pipe", "pipe"],
    });
  } catch (err) {
    stderr = err.stderr;
  }
  if (stderr && stderr.indexOf("musl") > -1) {
    return true;
  }
  return false;
}

const PLATFORMS = {
  win32: {
    x64: "@gale-linter/cli-win32-x64/gale.exe",
  },
  darwin: {
    x64: "@gale-linter/cli-darwin-x64/gale",
    arm64: "@gale-linter/cli-darwin-arm64/gale",
  },
  linux: {
    x64: "@gale-linter/cli-linux-x64/gale",
    arm64: "@gale-linter/cli-linux-arm64/gale",
  },
  "linux-musl": {
    x64: "@gale-linter/cli-linux-x64-musl/gale",
    arm64: "@gale-linter/cli-linux-arm64-musl/gale",
  },
};

const platform = os.platform();
const arch = os.arch();
const platformKey =
  platform === "linux" && isMusl() ? "linux-musl" : platform;
const packagePath = PLATFORMS[platformKey]?.[arch];

if (!packagePath) {
  console.warn(
    `[gale] Warning: Unsupported platform ${platform}-${arch}. ` +
      "The gale binary will not be available via npm.\n" +
      "You can build from source with: cargo install gale",
  );
  process.exit(0);
}

let binaryPath;
try {
  binaryPath = require.resolve(packagePath);
} catch {
  console.warn(
    `[gale] Warning: Platform package for ${platform}-${arch} was not installed.\n` +
      "This may happen if your package manager does not install optional dependencies.\n" +
      "You can set the GALE_BINARY environment variable to point to a Gale binary.",
  );
  process.exit(0);
}

// Verify the binary actually works
const result = spawnSync(binaryPath, ["--version"], {
  stdio: "pipe",
  encoding: "utf-8",
});

if (result.status === 0) {
  console.log(`[gale] Installed successfully: ${result.stdout.trim()}`);
} else {
  console.warn(
    "[gale] Warning: Binary found but failed to execute.\n" +
      `  Path: ${binaryPath}\n` +
      `  Error: ${result.stderr || result.error || "unknown"}\n` +
      "You can set the GALE_BINARY environment variable to point to a working Gale binary.",
  );
}
