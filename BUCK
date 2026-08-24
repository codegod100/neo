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
# anywhere useful on this machine - `-l`
# embedding doesn't carry `-L` along with it, so the actual link step below
# needs the Nix environment's unified library path passed explicitly.
GTK4_LIB_DIRS = [
    "-L/nix/store/8g01hy2crkmnjb5v1b6nmgbfrs6y4h39-gtk4-4.22.4/lib",
    "-L/nix/store/8hsq7gjh01g3ssi81rvwyyjlx7bj0ppv-libadwaita-1.9.3/lib",
    "-L/nix/store/wpwksxna2lp2y0px8xnxwgs0rfary570-glib-2.88.3/lib",
    "-L/nix/store/x1ch4nykvydrxgf650s4rrrfi4n03gil-pango-1.57.1/lib",
    "-L/nix/store/yx1n1yx1zzk09pw7cbivvbsb5z7q45ci-harfbuzz-13.2.1/lib",
    "-L/nix/store/xwmccxlk1hnmbkrigdnxgjh7caljkcfp-cairo-1.18.4/lib",
    "-L/nix/store/9qkivj7kz2zh9s847aklqlr0nb4g5yql-graphene-1.10.8/lib",
    "-L/nix/store/0y9x1nyg7wzdj3illkajsyh6ajrxbw3s-gdk-pixbuf-2.44.7/lib",
]

ARELM_ROOT = read_root_config("neo", "arelm_root", "../arelm")

ANDROID_AARCH64_LIB_DIRS = [
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/gtk/gtk",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/libadwaita/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/glib/glib",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/glib/gobject",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/glib/gio",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/pango/pango",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/harfbuzz/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/cairo/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/cairo/util/cairo-gobject",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/gdk-pixbuf/gdk-pixbuf",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/graphene/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-aarch64/subprojects/proxy-libintl-0.5",
]

ANDROID_X86_64_LIB_DIRS = [
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/gtk/gtk",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/libadwaita/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/glib/glib",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/glib/gobject",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/glib/gio",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/pango/pango",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/harfbuzz/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/cairo/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/cairo/util/cairo-gobject",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/gdk-pixbuf/gdk-pixbuf",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/graphene/src",
    "-L" + ARELM_ROOT + "/.pixiewood/bin-x86_64/subprojects/proxy-libintl-0.5",
]

PLATFORM_LIB_DIRS = select({
    "//platforms:is-android-aarch64": ANDROID_AARCH64_LIB_DIRS,
    "//platforms:is-android-x86_64": ANDROID_X86_64_LIB_DIRS,
    "DEFAULT": GTK4_LIB_DIRS,
})

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
    "//third-party:jni",
    "//third-party:log",
    "//third-party:matrix-sdk",
    "//third-party:matrix-sdk-ui",
    "//third-party:relm4",
    "//third-party:rusqlite",
    "//third-party:serde",
    "//third-party:serde_json",
    "//third-party:tokio",
    "//third-party:url",
]

NEO_SRCS = glob(["src/**/*.rs"]) + ["src/ui/app.ui"]

# Validates the authoritative Blueprint source and produces the same Builder XML
# checked in at `src/ui/app.ui`. The checked-in copy is what `include_str!` embeds
# because Buck's Rust rule does not preserve a genrule output beside `src/ui.rs`
# for relative include paths.
genrule(
    name = "blueprint-ui",
    srcs = ["src/ui/app.blp"],
    out = "app.ui",
    cmd = "blueprint-compiler compile --output $OUT $SRCS",
)

rust_library(
    name = "neo-lib",
    # `app.ui` is generated from `app.blp` with blueprint-compiler and read by
    # `src/ui.rs` through include_str!, so it must be an explicit sandbox input.
    srcs = NEO_SRCS,
    crate = "neo",
    crate_root = "src/lib.rs",
    edition = "2021",
    rustc_flags = PLATFORM_LIB_DIRS + RUSTC_RELEASE_FLAGS,
    visibility = ["PUBLIC"],
    deps = DEPS,
)

rust_binary(
    name = "neo",
    srcs = ["src/main.rs"],
    crate = "neo",
    crate_root = "src/main.rs",
    edition = "2021",
    rustc_flags = PLATFORM_LIB_DIRS + RUSTC_RELEASE_FLAGS,
    visibility = ["PUBLIC"],
    deps = [":neo-lib"],
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
    srcs = NEO_SRCS,
    crate = "neo",
    crate_root = "src/lib.rs",
    edition = "2021",
    rustc_flags = GTK4_LIB_DIRS + [],
    deps = DEPS + ["//third-party:wiremock"],
)
