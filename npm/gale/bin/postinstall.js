#!/usr/bin/env node

"use strict";

const { spawnSync } = require("child_process");
const os = require("os");

const PLATFORMS = {
  "darwin-arm64": "@aspect/gale-darwin-arm64/gale",
  "darwin-x64": "@aspect/gale-darwin-x64/gale",
  "linux-x64": "@aspect/gale-linux-x64/gale",
  "linux-arm64": "@aspect/gale-linux-arm64/gale",
  "win32-x64": "@aspect/gale-win32-x64/gale.exe",
};

const platform = `${os.platform()}-${os.arch()}`;
const packagePath = PLATFORMS[platform];

if (!packagePath) {
  console.warn(
    `[gale] Warning: Unsupported platform ${platform}. ` +
      "The gale binary will not be available."
  );
  process.exit(0);
}

let binaryPath;
try {
  binaryPath = require.resolve(packagePath);
} catch {
  console.warn(
    `[gale] Warning: Platform package for ${platform} was not installed. ` +
      "This may happen if your package manager does not install optional dependencies.\n" +
      "You can set the GALE_BINARY environment variable to point to a Gale binary."
  );
  process.exit(0);
}

// Verify the binary works
const result = spawnSync(binaryPath, ["--version"], {
  stdio: "pipe",
  encoding: "utf-8",
});

if (result.status === 0) {
  console.log(`[gale] Installed successfully: ${result.stdout.trim()}`);
} else {
  console.warn(
    `[gale] Warning: Binary found but failed to execute.\n` +
      `  Path: ${binaryPath}\n` +
      `  Error: ${result.stderr || result.error || "unknown"}\n` +
      "You can set the GALE_BINARY environment variable to point to a working Gale binary."
  );
}
