/// <reference types="vitest/config" />
import fs from "node:fs";
import path from "node:path";
import { defineConfig } from "vitest/config";

const appRoot = __dirname;
const testsRoot = path.resolve(appRoot, "tests/frontend");
const testSetupFile = path.join(testsRoot, "test_setup.ts");
const hasFrontendTests = fs.existsSync(testsRoot);

export default defineConfig({
	server: {
		fs: {
			allow: hasFrontendTests ? [appRoot, testsRoot] : [appRoot],
		},
	},
	resolve: {
		alias: {
			"@testing-library/react": path.resolve(appRoot, "./node_modules/@testing-library/react"),
			"@": path.resolve(appRoot, "./src"),
			react: path.resolve(appRoot, "./node_modules/react"),
			"react-dom": path.resolve(appRoot, "./node_modules/react-dom"),
			"react/jsx-dev-runtime": path.resolve(appRoot, "./node_modules/react/jsx-dev-runtime.js"),
			"react/jsx-runtime": path.resolve(appRoot, "./node_modules/react/jsx-runtime.js"),
		},
	},
	test: {
		globals: true,
		environment: "jsdom",
		passWithNoTests: true,
		setupFiles: hasFrontendTests && fs.existsSync(testSetupFile) ? [testSetupFile] : [],
		include: ["tests/frontend/test_*.test.ts?(x)"],
		coverage: {
			provider: "v8",
			reporter: ["text", "html"],
		},
	},
});
