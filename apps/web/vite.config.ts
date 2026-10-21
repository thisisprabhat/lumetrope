/// <reference types="vitest" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createReadStream, existsSync } from "node:fs";
import { resolve } from "node:path";

// Dev only: serve the git-ignored LottieFiles test samples without copying them into builds.
const localSamples = {
  name: "lumetrope-local-samples",
  configureServer(server: any) {
    const dir = resolve(__dirname, "../../samples/lottiefiles");
    server.middlewares.use("/samples/lottiefiles", (req: any, res: any, next: any) => {
      const file = resolve(dir, decodeURIComponent((req.url ?? "/").slice(1).split("?")[0]));
      if (!file.startsWith(dir) || !existsSync(file)) return next();
      res.setHeader("Content-Type", "application/zip");
      createReadStream(file).pipe(res);
    });
  },
};

export default defineConfig({
  plugins: [react(), localSamples],
  server: { port: 5173, strictPort: false },
  build: { target: "es2022" },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
