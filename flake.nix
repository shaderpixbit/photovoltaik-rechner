{
  description = "Photovoltaik — Tauri v2 + Svelte 5 development shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # ── Rust toolchain (stable, with the components Tauri devs reach for)
        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" "clippy" "rustfmt" ];
        };

        # ── GTK / WebKit stack required by Tauri v2 on Linux.
        #     Tauri v2 uses webkit2gtk 4.1 + libsoup 3 (not 4.0/2).
        tauriLibs = with pkgs; [
          webkitgtk_4_1
          gtk3
          libsoup_3
          glib
          glib-networking
          gdk-pixbuf
          gobject-introspection
          cairo
          pango
          atk
          librsvg
          openssl
          xdotool
          libayatana-appindicator
        ];

        # Native command-line helpers — toolchains, bundlers, packagers.
        devTools = with pkgs; [
          rust
          bun
          nodejs_22       # svelte-check + sveltekit-CLI brauchen ein node-Binary
          pkg-config
          wrapGAppsHook3
          # Linux-Bundler für `tauri build`. AppImage / DMG werden von tauri-cli
          # selbst beschafft, wir liefern nur was Debian-/RPM-Distros brauchen.
          dpkg            # .deb
          rpm             # .rpm
          # Python für den Anker-Cloud-Sidecar (vendor-import-anker/).
          # anker-solix-api verlangt Python >=3.12 — daher explizit gepinnt.
          # build-sidecar.sh legt ein lokales .venv an und installiert die
          # Lib direkt aus dem GitHub-Tag (sie ist nicht auf PyPI).
          # gcc als Fallback, falls eine Wheel-Dependency nativ kompiliert werden muss.
          python312
          gcc
          # Convenience:
          jq
          git
          sqlite          # CLI für photovoltaik.db beim Debuggen
        ];
      in
      {
        # ──────────────────────────────────────────────────────────────────
        # nix develop  (oder:  direnv allow  mit dem .envrc-Snippet)
        # ──────────────────────────────────────────────────────────────────
        devShells.default = pkgs.mkShell {
          name = "photovoltaik";

          nativeBuildInputs = devTools;
          buildInputs = tauriLibs;

          shellHook = ''
            export PKG_CONFIG_PATH="${pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" tauriLibs}:$PKG_CONFIG_PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath tauriLibs}:''${LD_LIBRARY_PATH:-}"
            export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules"
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:''${XDG_DATA_DIRS:-}"

            # Cargo-Cache lokal im Projekt halten, damit parallele Shells sich
            # nicht ins Gehege kommen.
            export CARGO_HOME="$PWD/.cargo"
            export PATH="$CARGO_HOME/bin:$PATH"

            cat <<EOF
            ┌─────────────────────────────────────────────┐
            │  Photovoltaik — dev shell ready             │
            ├─────────────────────────────────────────────┤
            │  rust    $(rustc --version | cut -d' ' -f2)
            │  bun     $(bun --version)
            │  node    $(node --version)
            │  python  $(python3 --version | cut -d' ' -f2)
            │  webkit  4.1   ·   libsoup 3   ·   gtk 3
            ├─────────────────────────────────────────────┤
            │  bun install            install JS deps
            │  bun run tauri dev      start desktop app
            │  bun run sidecar:build  build Anker sidecar
            │  bun run tauri:release  bundle installer
            └─────────────────────────────────────────────┘
            EOF
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
