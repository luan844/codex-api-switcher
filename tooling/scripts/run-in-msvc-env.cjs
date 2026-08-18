const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFileSync, spawnSync } = require("child_process");

function quoteForCmd(value) {
  if (/^[A-Za-z0-9_./:-]+$/.test(value)) {
    return value;
  }

  return `"${value.replace(/%/g, "%%").replace(/"/g, '""')}"`;
}

function findExistingPaths(candidates) {
  return candidates.filter((candidate) => fs.existsSync(candidate));
}

function getProgramFilesRoots() {
  return [...new Set([process.env.ProgramFiles, process.env["ProgramFiles(x86)"]].filter(Boolean))];
}

function getVsInstances() {
  const vswherePath = findExistingPaths(
    getProgramFilesRoots().map((root) =>
      path.join(root, "Microsoft Visual Studio", "Installer", "vswhere.exe"),
    ),
  )[0];

  if (!vswherePath) {
    return [];
  }

  try {
    const output = execFileSync(
      vswherePath,
      ["-all", "-products", "*", "-requires", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-format", "json"],
      { encoding: "utf8" },
    );

    const instances = JSON.parse(output);
    return Array.isArray(instances) ? instances : [];
  } catch {
    return [];
  }
}

function hasCompleteMsvcInclude(installationPath) {
  const msvcRoot = path.join(installationPath, "VC", "Tools", "MSVC");
  if (!fs.existsSync(msvcRoot)) {
    return false;
  }

  const versions = fs
    .readdirSync(msvcRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left));

  return versions.some((version) => {
    const includeDir = path.join(msvcRoot, version, "include");
    return (
      fs.existsSync(path.join(includeDir, "vcruntime.h")) &&
      fs.existsSync(path.join(includeDir, "excpt.h"))
    );
  });
}

function buildFallbackVcvarsCandidates() {
  const versions = ["18", "2022", "2019"];
  const editions = ["Community", "BuildTools"];

  return getProgramFilesRoots().flatMap((root) =>
    versions.flatMap((version) =>
      editions.map((edition) =>
        path.join(
          root,
          "Microsoft Visual Studio",
          version,
          edition,
          "VC",
          "Auxiliary",
          "Build",
          "vcvars64.bat",
        ),
      ),
    ),
  );
}

function resolveVcVars64() {
  const instancePaths = getVsInstances()
    .map((instance) => instance.installationPath)
    .filter(Boolean)
    .filter(hasCompleteMsvcInclude)
    .map((installationPath) => path.join(installationPath, "VC", "Auxiliary", "Build", "vcvars64.bat"));

  const fallbackPaths = buildFallbackVcvarsCandidates();

  return findExistingPaths([...instancePaths, ...fallbackPaths]).find((candidate) =>
    hasCompleteMsvcInclude(path.resolve(candidate, "..", "..", "..", "..")),
  );
}

function runDirect(commandArgs) {
  const result = spawnSync(commandArgs[0], commandArgs.slice(1), {
    stdio: "inherit",
    shell: false,
  });

  if (result.error) {
    throw result.error;
  }

  process.exit(result.status ?? 1);
}

const commandArgs = process.argv.slice(2);

if (commandArgs.length === 0) {
  console.error("缺少要执行的命令。");
  process.exit(1);
}

if (process.platform !== "win32") {
  runDirect(commandArgs);
}

const vcvars64 = resolveVcVars64();

if (!vcvars64) {
  console.error("未找到可用的 MSVC C++ 环境，请安装带 C++ 工具链的 Visual Studio 或 Build Tools。");
  process.exit(1);
}

const commandText = commandArgs.map(quoteForCmd).join(" ");

const tempScriptPath = path.join(os.tmpdir(), `codex-switch-msvc-${process.pid}.cmd`);
fs.writeFileSync(
  tempScriptPath,
  `@echo off\r\ncall "${vcvars64}"\r\nif errorlevel 1 exit /b %errorlevel%\r\nset "PATH=%USERPROFILE%\\.cargo\\bin;%PATH%"\r\n${commandText}\r\n`,
  "utf8",
);

const result = spawnSync("cmd.exe", ["/d", "/c", tempScriptPath], {
  stdio: "inherit",
});

try {
  fs.unlinkSync(tempScriptPath);
} catch {
  // 忽略临时脚本清理失败
}

if (result.error) {
  throw result.error;
}

process.exit(result.status ?? 1);