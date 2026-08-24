# Project rules

## Build system

- Use Buck2 for all builds and tests in this repository. Do not invoke
  `cargo build`, `cargo check`, `cargo run`, or `cargo test`; Cargo files are
  dependency-resolution inputs, not the project's build interface.
- Use the repository's BuildBuddy remote execution (RBE) platform for every
  Buck2 command by passing
  `--config build.execution_platforms=root//platforms:remote`. Do not rely on
  `--prefer-remote` alone: the default execution platform is local.
- Build or type-check the application with
  `buck2 build --config build.execution_platforms=root//platforms:remote //:neo`.
- Run the application with
  `buck2 run --config build.execution_platforms=root//platforms:remote //:neo`.
- Run tests with
  `buck2 test --config build.execution_platforms=root//platforms:remote //:neo-test`.
- After changing `src/ui/app.blp`, regenerate `src/ui/app.ui` with
  `blueprint-compiler compile --output src/ui/app.ui src/ui/app.blp` and
  validate it with
  `buck2 build --config build.execution_platforms=root//platforms:remote //:blueprint-ui`.
