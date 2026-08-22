# Execution platforms for BuildBuddy cache-only and remote-execution modes.
# The remote platform executes directly on a persistent Nix-enabled worker.
# Snowydeer supplies the pinned toolchain and /opt/neo-nix exposes the same
# closure to Cargo build scripts which discover native tools via PATH.
#
# These are selected via `execution_platforms` in .buckconfig.local (machine-
# local, gitignored). The tracked .buckconfig remains fully local so a plain
# clone does not require BuildBuddy credentials.

load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")

def _cache_execution_platform_impl(ctx: AnalysisContext) -> list[Provider]:
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    cfg = ConfigurationInfo(constraints = constraints, values = {})

    name = ctx.label.raw_target()
    platform = ExecutionPlatformInfo(
        label = name,
        configuration = cfg,
        executor_config = CommandExecutorConfig(
            local_enabled = True,
            remote_enabled = False,
            remote_cache_enabled = True,
            allow_cache_uploads = True,
            use_windows_path_separators = ctx.attrs.use_windows_path_separators,
        ),
    )

    return [
        DefaultInfo(),
        platform,
        PlatformInfo(label = str(name), configuration = cfg),
        ExecutionPlatformRegistrationInfo(
            platforms = [platform],
            exec_marker_constraint = get_exec_platform_marker(),
        ),
    ]

cache_execution_platform = rule(
    impl = _cache_execution_platform_impl,
    attrs = {
        "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "use_windows_path_separators": attrs.bool(),
    },
)

def _buildbuddy_execution_platform_impl(ctx: AnalysisContext) -> list[Provider]:
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    cfg = ConfigurationInfo(constraints = constraints, values = {})

    name = ctx.label.raw_target()
    platform = ExecutionPlatformInfo(
        label = name,
        configuration = cfg,
        executor_config = CommandExecutorConfig(
            local_enabled = False,
            remote_enabled = True,
            use_limited_hybrid = False,
            remote_cache_enabled = True,
            allow_cache_uploads = True,
            remote_execution_properties = {
                "OSFamily": "Linux",
                "Pool": "neo-rbe",
                "env-overrides": ",".join([
                    "PATH=/opt/neo-nix/bin:/root/.nix-profile/bin:/usr/bin:/bin",
                    "PKG_CONFIG_PATH=/opt/neo-nix/lib/pkgconfig:/opt/neo-nix/share/pkgconfig",
                    "LIBRARY_PATH=/opt/neo-nix/lib",
                    "CPATH=/opt/neo-nix/include",
                ]),
                "workload-isolation-type": "none",
                "use-self-hosted-executors": "true",
            },
            remote_execution_use_case = "buck2-default",
            remote_output_paths = "output_paths",
            use_windows_path_separators = ctx.attrs.use_windows_path_separators,
        ),
    )

    return [
        DefaultInfo(),
        platform,
        PlatformInfo(label = str(name), configuration = cfg),
        ExecutionPlatformRegistrationInfo(
            platforms = [platform],
            exec_marker_constraint = get_exec_platform_marker(),
        ),
    ]

buildbuddy_execution_platform = rule(
    impl = _buildbuddy_execution_platform_impl,
    attrs = {
        "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "use_windows_path_separators": attrs.bool(),
    },
)
