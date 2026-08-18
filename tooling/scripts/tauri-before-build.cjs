const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const repoRoot = path.resolve(__dirname, "..", "..");
const distDir = path.join(repoRoot, "dist");
const hasDist = fs.existsSync(distDir);

if (process.env.SKIP_TAURI_FRONTEND_BUILD === "1") {
  if (!hasDist) {
    console.error("检测到已跳过前端构建，但 dist 目录不存在。请先准备前端产物。");
    process.exit(1);
  }

  process.exit(0);
}

if (process.env.CI === "true" && hasDist) {
  process.exit(0);
}

const command = process.platform === "win32" ? (process.env.ComSpec || "cmd.exe") : "npm";
const args =
  process.platform === "win32"
    ? ["/d", "/c", "npm run build:dist"]
    : ["run", "build:dist"];

const result = spawnSync(command, args, {
  cwd: repoRoot,
  stdio: "inherit",
  shell: false,
});

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);