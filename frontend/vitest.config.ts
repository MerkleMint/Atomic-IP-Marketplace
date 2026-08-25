import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/setup.ts"],
    // SwapStatusDashboard.*.test.tsx predate this test runner: they target
    // Jest APIs (jest.mock/jest.fn) with no Jest install and no config in the
    // repo, so they have never been runnable. Left as-is — fixing them is
    // unrelated to this change.
    exclude: [
      "**/node_modules/**",
      "tests/SwapStatusDashboard.test.tsx",
      "tests/SwapStatusDashboard.integration.test.tsx",
    ],
  },
});
