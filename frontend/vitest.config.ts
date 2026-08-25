import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    // Scoped to src/: the pre-existing frontend/tests/ directory predates any
    // test runner being wired up in this project and is not part of this fix.
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
});
