# SPDX-FileCopyrightText: 2026 Mercury Technologies, Inc.
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Adapted from MercuryTechnologies/snowydeer
# toolchains/nix/nix_build.bzl. Unlike upstream's workstation-oriented rule,
# this variant realizes Nix paths remotely on Neo's persistent bare executor.

def _nix_build_impl(ctx: AnalysisContext) -> list[Provider]:
    output = ctx.actions.declare_output("nix-environment", dir = True)
    flake = ctx.attrs.flake
    attr = ctx.attrs.attr or ctx.label.name

    binaries = cmd_args(ctx.attrs.binaries)
    ctx.actions.run(
        cmd_args(
            "bash",
            "-ec",
            """
            flake=$(nix store add-path --name source "$1")
            store_path=$(nix build --print-build-logs --show-trace \
                --no-update-lock-file --no-use-registries \
                --no-link --print-out-paths "path:$flake#$2")
            output="$3"
            mkdir -p "$output/bin"
            printf '%s\n' "$store_path" > "$output/store-path"
            shift 3
            for binary in "$@"; do
                if [[ ! -x "$store_path/bin/$binary" ]]; then
                    echo "snowydeer: $store_path/bin/$binary is not executable" >&2
                    exit 1
                fi
                printf '#!/bin/sh\nexec %s/bin/%s "$@"\n' \
                    "$store_path" "$binary" > "$output/bin/$binary"
                chmod +x "$output/bin/$binary"
            done
            """,
            "--",
            flake,
            attr,
            output.as_output(),
            binaries,
        ),
        category = "nix_build",
        allow_cache_upload = False,
    )

    sub_targets = {
        binary: [
            DefaultInfo(default_output = output),
            RunInfo(args = cmd_args(output, "bin", binary, delimiter = "/")),
        ]
        for binary in ctx.attrs.binaries
    }

    return [
        DefaultInfo(
            default_output = output,
            sub_targets = sub_targets,
        ),
    ]

nix_build = rule(
    impl = _nix_build_impl,
    attrs = {
        "attr": attrs.option(attrs.string(), default = None),
        "binaries": attrs.list(attrs.string(), default = []),
        "flake": attrs.source(allow_directory = True),
    },
)
