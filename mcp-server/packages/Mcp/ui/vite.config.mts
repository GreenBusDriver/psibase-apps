import path from "path";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const appDirectory = path.resolve(__dirname);

// Standalone UI for out-of-tree SDK apps. At runtime on a booted chain,
// @psibase/common-lib is served from /common/common-lib.js (Sites/CommonApi).
export default defineConfig({
    plugins: [react(), tailwindcss()],
    resolve: {
        alias: {
            "@shared": path.resolve(appDirectory, "shared"),
        },
    },
    build: {
        minify: true,
        sourcemap: false,
        rollupOptions: {
            input: {
                main: path.resolve(appDirectory, "index.html"),
                agent: path.resolve(appDirectory, "agent.html"),
            },
            external: ["@psibase/common-lib"],
            output: {
                paths: {
                    "@psibase/common-lib": "/common/common-lib.js",
                },
            },
        },
    },
});
