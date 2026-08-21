# A copy of prelude//platforms:default's execution platform (see
# buck-out/.../external_cells/bundled/prelude/platforms/defs.bzl) that also
# turns on remote action-cache read/write. Actions still execute locally -
# `remote_enabled` stays False - only cache lookups/uploads go over the
# network to BuildBuddy. See toolchains/BUCK (absolute clang/ar paths) and
# BUCK's GTK4_LIB_DIRS (Homebrew Cellar paths) for why real remote
# *execution* would break: this machine's toolchain/libraries don't exist
# on generic RE workers.
#
# Only wired in via `execution_platforms` in .buckconfig.local (machine-
# local, gitignored) - the tracked .buckconfig keeps using
# prelude//platforms:default so a plain clone still builds fully local
# with no BuildBuddy dependency.

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
