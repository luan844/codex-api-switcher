import path from "node:path";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

export default defineConfig({
	plugins: [react(), tailwindcss()],
	clearScreen: false,
	server: {
		port: 1420,
		strictPort: true,
		watch: {
			ignored: ["**/src-tauri/target/**"],
		},
	},
	envPrefix: "VITE_",
	resolve: {
		alias: {
			"@": path.resolve(__dirname, "./src"),
		},
	},
	build: {
		target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
		minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
		sourcemap: !!process.env.TAURI_DEBUG,
		rolldownOptions: {
			checks: {
				pluginTimings: false,
			},
			output: {
				manualChunks(id) {
					if (!id.includes("node_modules")) {
						return undefined;
					}
					if (
						id.includes("react") ||
						id.includes("react-dom") ||
						id.includes("scheduler")
					) {
						return "vendor-react";
					}
					if (
						id.includes("@radix-ui") ||
						id.includes("lucide-react") ||
						id.includes("sonner")
					) {
						return "vendor-ui";
					}
					if (
						id.includes("react-hook-form") ||
						id.includes("@hookform") ||
						id.includes("zod")
					) {
						return "vendor-form";
					}
					if (id.includes("@tauri-apps")) {
						return "vendor-tauri";
					}
					return "vendor";
				},
			},
		},
	},
});
