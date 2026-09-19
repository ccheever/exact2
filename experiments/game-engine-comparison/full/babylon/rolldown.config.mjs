export default {
  input: "src/main.js",
  output: {
    dir: "dist",
    entryFileNames: "game.js",
    format: "esm",
    minify: true,
    sourcemap: false,
  },
};
