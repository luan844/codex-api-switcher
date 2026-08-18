const fs = require("fs");
const path = require("path");

const rootDir = path.resolve(__dirname, "..", "..");
const includeNodeModules = process.argv.includes("--with-node-modules");
const dryRun = process.argv.includes("--dry-run");

const targets = [
  "dist",
  "coverage",
  ".vite",
  ".turbo",
  ".parcel-cache",
  ".cache",
  path.join("src-tauri", "target"),
  path.join("src-tauri", "bundle"),
];

if (includeNodeModules) {
  targets.push("node_modules");
}

function formatSize(bytes) {
  if (!bytes || bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
}

function getSizeBytes(targetPath) {
  const stats = fs.statSync(targetPath);
  if (!stats.isDirectory()) {
    return stats.size;
  }

  let total = 0;
  for (const entry of fs.readdirSync(targetPath, { withFileTypes: true })) {
    total += getSizeBytes(path.join(targetPath, entry.name));
  }

  return total;
}

let removedCount = 0;
let freedBytes = 0;

for (const relativeTarget of targets) {
  const targetPath = path.join(rootDir, relativeTarget);
  if (!fs.existsSync(targetPath)) {
    console.log(`跳过 ${relativeTarget}（不存在）`);
    continue;
  }

  const sizeBytes = getSizeBytes(targetPath);
  if (dryRun) {
    console.log(`计划删除 ${relativeTarget}，预计释放 ${formatSize(sizeBytes)}`);
    continue;
  }

  fs.rmSync(targetPath, { recursive: true, force: true, maxRetries: 2 });
  removedCount += 1;
  freedBytes += sizeBytes;
  console.log(`已删除 ${relativeTarget}，释放 ${formatSize(sizeBytes)}`);
}

if (dryRun) {
  console.log("试运行完成，未执行实际删除。");
  process.exit(0);
}

console.log(`完成，共删除 ${removedCount} 个目录，释放 ${formatSize(freedBytes)}`);