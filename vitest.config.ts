import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: false,
    clearMocks: true,
    setupFiles: ["./vitest.setup.ts"],
  },
});
