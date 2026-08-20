# `def`/`rule()` aren't allowed in BUCK files (only in .bzl, a stricter
# dialect restriction than plain Starlark) - this one small piece of
# @prelude//toolchains/demo.bzl's `system_demo_toolchains()` macro body has
# to live here instead of directly in toolchains/BUCK for that reason; see
# toolchains/BUCK's top-of-file comment for why the rest of the macro is
# inlined there rather than called as a macro.
def _hack_impl(ctx):
    return ctx.attrs.actual.providers

android_hack_alias = rule(
    impl = _hack_impl,
    attrs = {
        "actual": attrs.toolchain_dep(),
    },
)
