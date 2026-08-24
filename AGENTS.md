# Project rules

## Build system

- Use Buck2 for all builds and tests in this repository. Do not invoke
  `cargo build`, `cargo check`, `cargo run`, or `cargo test`; Cargo files are
  dependency-resolution inputs, not the project's build interface.
- Use the repository's BuildBuddy remote execution (RBE) platform, configured
  as the default execution platform in `.buckconfig`.
- Build or type-check the application with
  `buck2 build //:neo`.
- Run the application with
  `buck2 run //:neo`.
- Run tests with
  `buck2 test //:neo-test`.
- After changing `src/ui/app.blp`, regenerate `src/ui/app.ui` with
  `blueprint-compiler compile --output src/ui/app.ui src/ui/app.blp` and
  validate it with
  `buck2 build //:blueprint-ui`.
