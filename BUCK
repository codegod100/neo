# neo's own crate: a single `rust_binary` (mirrors Cargo.toml's single
# `[[bin]]` - unlike arelm, there's no `[lib]`/cdylib split here since neo is
# desktop-only, no Android/JNI entrypoint to build).
#
#  - `buck2 run //:neo` - the desktop binary, replacing `cargo run`.
#
# Depends on third-party/BUCK, the reindeer-buckified crate graph
# reindeer.toml describes - one dependency edge, `:relm4`, pulls in the
# entire gtk4-rs + libadwaita stack transitively, and `:matrix-sdk` pulls in
# the sqlite store / rustls / e2e-encryption stack. Both are reindeer's own
# generated public aliases for the crate (not the version-suffixed target,
# which defaults to private).
#
# The gtk4-sys/glib-sys/libadwaita-sys/etc fixups (third-party/fixups/*/
# fixups.toml) turn on `rustc_link_lib` so each -sys crate's
# `cargo:rustc-link-lib=gtk-4` etc build.rs output becomes real `-l` flags on
# *that* crate's own rustc invocation - rustc embeds those as native-library
# requirements in the rlib's metadata, so they propagate automatically to
# whatever finally links against it (no `-l` flags needed here). But
# system-deps' build.rs never emits `cargo:rustc-link-search` pointing
# anywhere useful on this machine (GTK4/libadwaita only exist under
# Homebrew's non-standard Cellar paths, not a linker default dir) - `-l`
# embedding doesn't carry `-L` along with it, so the actual link step below
# needs those search paths passed explicitly (pkg-config itself doesn't need
# this on this machine - see README.md's "Building" section on why).
GTK4_LIB_DIRS = [
    "-L/home/linuxbrew/.linuxbrew/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/libadwaita/1.9.3/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/gtk4/4.22.4/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/pango/1.58.2/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/harfbuzz/14.3.1/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/cairo/1.18.4/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/graphene/1.10.8/lib",
    "-L/home/linuxbrew/.linuxbrew/Cellar/glib/2.88.3/lib",
]

# buck2 has no built-in cargo-style dev/release profile concept - `buck2
# build --config neo.release=true //:neo` mirrors cargo's `--release`
# (`-Copt-level=3`) closely enough for local dev without trying to replicate
# Cargo.toml's full `[profile.dev]`/`[profile.release]` split (line-tables-
# only debuginfo, LTO, etc are left at rustc's stock settings here).
RUSTC_RELEASE_FLAGS = ["-Copt-level=3"] if read_root_config("neo", "release", "false") == "true" else []

# Shared between `:neo` and `:neo-test` below (must mirror Cargo.toml's
# [dependencies] exactly - `:neo-test` compiles the same crate_root, so a dep
# missing here fails *both* targets, not just one, the same way a missing
# `cargo` dependency would fail `cargo build`/`cargo test` alike).
DEPS = [
    "//third-party:anyhow",
    "//third-party:chrono",
    "//third-party:dirs",
    "//third-party:env_logger",
    "//third-party:eyeball-im",
    "//third-party:futures-util",
    "//third-party:log",
    "//third-party:matrix-sdk",
    "//third-party:matrix-sdk-ui",
    "//third-party:open",
    "//third-party:relm4",
    "//third-party:rusqlite",
    "//third-party:serde",
    "//third-party:serde_json",
    "//third-party:tokio",
    "//third-party:url",
]

rust_binary(
    name = "neo",
    srcs = glob(["src/**/*.rs"]),
    crate = "neo",
    crate_root = "src/main.rs",
    edition = "2021",
    rustc_flags = GTK4_LIB_DIRS + RUSTC_RELEASE_FLAGS,
    visibility = ["PUBLIC"],
    deps = DEPS,
)

# `buck2 test //:neo-test` - replaces `cargo test`. Same crate_root/srcs as
# `:neo` above (mirrors how `cargo test` recompiles a bin target with a
# `--test` harness rather than using a separate crate) plus `wiremock`, whose
# `#[cfg(test)] mod tests` in src/matrix_bridge.rs are the only tests in the
# crate today. `wiremock` isn't listed in `DEPS` above because it's a normal
# (not dev-) dependency purely for reindeer/buck2 target-kind reasons - see
# Cargo.toml's comment on it and on matrix-sdk's `testing` feature - it's
# still only ever *used* from #[cfg(test)] code, same as it would be as a
# real dev-dependency.
rust_test(
    name = "neo-test",
    srcs = glob(["src/**/*.rs"]),
    crate = "neo",
    crate_root = "src/main.rs",
    edition = "2021",
    rustc_flags = GTK4_LIB_DIRS,
    deps = DEPS + ["//third-party:wiremock"],
)
