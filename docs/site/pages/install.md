# Install the tools

Install Bun, Git, rustup and Chromium before setting up Exact. These steps were run on macOS on Apple Silicon. Linux supports the web loop and the Linux host; macOS and iOS builds require a Mac with Xcode.

## Bun, Git and Rust

- Install [Bun](https://bun.sh) **1.4.2**, the version Exact pins. Node and npm are not required.
- Install [Git](https://git-scm.com/downloads) to clone Exact.
- Install [rustup](https://rustup.rs) yourself. It manages the Rust toolchains Exact uses.

On macOS or Linux, install the pinned Bun beside any existing version:

```sh
curl -fsSL https://bun.sh/install | BUN_INSTALL=~/.bun-1.4.2 bash -s bun-v1.4.2
export PATH="$HOME/.bun-1.4.2/bin:$PATH"
bun --version
```

The version should be `1.4.2`. Keep this Bun on your `PATH` for the commands below.

## Chromium

The agent uses Chromium to drive and test web apps. Install Playwright's Chromium and its headless shell with the version pinned by Exact:

```sh
bunx --bun playwright@1.64.1 install chromium chromium-headless-shell
```

Exact uses the headless shell by default and the full browser for features that need it, such as GPU rendering.

Alternatively, install [Google Chrome](https://www.google.com/chrome/) and set `CHROME` to its executable. On macOS, the usual path is:

```sh
export CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
```

On Linux, set `CHROME` to the installed browser's executable path or command name.

## Set up Exact

Follow [Set up Exact](home.md) to clone Exact and create your first app. Its `bun scripts/exact.mjs setup` command installs the pinned Rust toolchains, components and targets, wasm-bindgen, Binaryen, Bun dependencies and the Hermes bundles for the host and supported Apple targets. It also fetches the locked Rust dependencies.

Setup does not install rustup or a browser. From the Exact checkout, `bun scripts/exact.mjs setup --check` checks the build prerequisites without installing them.
