const { defineConfig } = require("eslint/config");
const expo = require("eslint-config-expo/flat");

module.exports = defineConfig([
  expo,
  { files: ["**/*.cjs"], languageOptions: { sourceType: "commonjs", globals: { __dirname: "readonly" } } },
  { ignores: ["dist/**", ".expo/**"] },
]);
