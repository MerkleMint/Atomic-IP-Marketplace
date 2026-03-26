import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  define: {
    // stellar-wallets-kit uses Buffer internally
    global: "globalThis",
  },
});
