# SPDX-FileCopyrightText: 2025 Meta Platforms, Inc.
# SPDX-FileCopyrightText: 2026 Mercury Technologies, Inc.
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Adapted from MercuryTechnologies/snowydeer
# toolchains/nix/nix_rust_toolchain.bzl.

load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")

def _nix_rust_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    nix_rust = ctx.attrs.nix_rust[DefaultInfo].sub_targets

    return [
        DefaultInfo(),
        RustToolchainInfo(
            allow_lints = ctx.attrs.allow_lints,
            clippy_driver = nix_rust["clippy-driver"][RunInfo],
            compiler = nix_rust["rustc"][RunInfo],
            default_edition = ctx.attrs.default_edition,
            deny_lints = ctx.attrs.deny_lints,
            doctests = ctx.attrs.doctests,
            nightly_features = False,
            panic_runtime = PanicRuntime("unwind"),
            report_unused_deps = False,
            rustc_binary_flags = [
                "-Clink-arg=-Wl,--dynamic-linker=/lib64/ld-linux-x86-64.so.2",
                "-Clink-arg=-Wl,-rpath,/opt/neo-nix/lib",
                "-Clink-arg=-Wl,-rpath,/home/linuxbrew/.linuxbrew/lib",
            ],
            rustc_flags = [],
            rustc_target_triple = "x86_64-unknown-linux-gnu",
            rustc_test_flags = [
                "-Clink-arg=-Wl,--dynamic-linker=/lib64/ld-linux-x86-64.so.2",
                "-Clink-arg=-Wl,-rpath,/opt/neo-nix/lib",
                "-Clink-arg=-Wl,-rpath,/home/linuxbrew/.linuxbrew/lib",
            ],
            rustdoc = nix_rust["rustdoc"][RunInfo],
            rustdoc_flags = [],
            sysroot_path = None,
            warn_lints = ctx.attrs.warn_lints,
        ),
    ]

nix_rust_toolchain = rule(
    impl = _nix_rust_toolchain_impl,
    attrs = {
        "allow_lints": attrs.list(attrs.string(), default = []),
        "default_edition": attrs.string(default = "2021"),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "doctests": attrs.bool(default = False),
        "nix_rust": attrs.dep(default = "root//nix:environment"),
        "warn_lints": attrs.list(attrs.string(), default = []),
    },
    is_toolchain_rule = True,
)
