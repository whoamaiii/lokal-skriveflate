import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    exclude: ["**/node_modules/**", "**/node_modules.bak*/**", "**/dist/**"],
    environment: "happy-dom",
    globals: false,
    clearMocks: true,
    pool: "threads",
    poolOptions: {
      threads: {
        singleThread: true,
      },
    },
    setupFiles: ["./vitest.setup.ts"],
  },
});
