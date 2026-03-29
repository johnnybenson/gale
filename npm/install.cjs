#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const https = require("https");

const REPO = "LyricalString/gale";
const VERSION = require("./package.json").version;

const PLATFORM_MAP = {
  "darwin-arm64": "gale-aarch64-apple-darwin",
  "darwin-x64": "gale-x86_64-apple-darwin",
  "linux-arm64": "gale-aarch64-unknown-linux-gnu",
  "linux-x64": "gale-x86_64-unknown-linux-gnu",
};

function getPlatformKey() {
  return `${process.platform}-${process.arch}`;
}

function getBinaryName() {
  const key = getPlatformKey();
  const name = PLATFORM_MAP[key];
  if (!name) {
    console.error(`Unsupported platform: ${key}`);
    console.error(`Supported: ${Object.keys(PLATFORM_MAP).join(", ")}`);
    process.exit(1);
  }
  return name;
}

function download(url) {
  return new Promise((resolve, reject) => {
    const follow = (url, redirects = 0) => {
      if (redirects > 5) return reject(new Error("Too many redirects"));

      https.get(url, { headers: { "User-Agent": "gale-npm" } }, (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return follow(res.headers.location, redirects + 1);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode} fetching ${url}`));
        }
        const chunks = [];
        res.on("data", (chunk) => chunks.push(chunk));
        res.on("end", () => resolve(Buffer.concat(chunks)));
        res.on("error", reject);
      }).on("error", reject);
    };
    follow(url);
  });
}

async function main() {
  const binaryName = getBinaryName();
  const binDir = path.join(__dirname, "bin");
  const binPath = path.join(binDir, "gale");

  // Skip if already installed (real binary, not placeholder)
  if (fs.existsSync(binPath)) {
    const stat = fs.statSync(binPath);
    if (stat.size > 1024) {
      return;
    }
  }

  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${binaryName}`;
  console.log(`Downloading Gale v${VERSION} for ${getPlatformKey()}...`);

  try {
    const data = await download(url);
    fs.mkdirSync(binDir, { recursive: true });
    fs.writeFileSync(binPath, data);
    fs.chmodSync(binPath, 0o755);
    console.log("Gale installed successfully.");
  } catch (err) {
    console.error(`Failed to download Gale: ${err.message}`);
    console.error(`URL: ${url}`);
    console.error(`You can install manually from: https://github.com/${REPO}/releases`);
    process.exit(1);
  }
}

main();
