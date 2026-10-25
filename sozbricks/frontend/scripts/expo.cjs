const { spawn } = require("node:child_process");
const { mkdirSync } = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const local = path.resolve(root, "..", ".local");
const temporary = path.join(local, "tmp");
mkdirSync(temporary, { recursive: true });
const cli = path.join(path.dirname(require.resolve("expo/package.json")), "bin", "cli");
const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], {
  cwd: root,
  stdio: "inherit",
  env: { ...process.env, TEMP: temporary, TMP: temporary, TMPDIR: temporary,
    npm_config_cache: path.join(local, "npm-cache"), EXPO_NO_TELEMETRY: "1" },
});
child.on("error", error => { console.error(error.message); process.exitCode = 1; });
child.on("exit", code => { process.exitCode = code ?? 1; });
process.on("SIGINT", () => child.kill("SIGINT"));
process.on("SIGTERM", () => child.kill("SIGTERM"));
