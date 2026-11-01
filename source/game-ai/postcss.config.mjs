import { fileURLToPath } from "node:url";

const config = {
  plugins: {
    "@tailwindcss/postcss": {
      // Public classes come from application source, never private notes or build artifacts.
      base: fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
};

export default config;
