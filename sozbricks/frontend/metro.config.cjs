const path = require("node:path");
const { getDefaultConfig } = require("expo/metro-config");
const { FileStore } = require("metro-cache");
const { mkdirSync } = require("node:fs");

const config = getDefaultConfig(__dirname);
config.maxWorkers = 2;
config.cacheStores = [new FileStore({ root: path.resolve(__dirname, "../.local/metro-cache") })];
config.fileMapCacheDirectory = path.resolve(__dirname, "../.local/metro-file-map");
mkdirSync(config.fileMapCacheDirectory, { recursive: true });
module.exports = config;
