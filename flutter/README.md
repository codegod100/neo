# neo (Flutter)

neo rebuilt with [Flutter](https://flutter.dev), drawn with
[flatkit](https://github.com/codegod100/flutterkit) widgets and talking to
Matrix through the Dart [`matrix`](https://pub.dev/packages/matrix) SDK. It
has the same screens as the Rust/GTK app in the repository root:

- **Connect**: password or browser SSO sign-in, with "Remember me on this
  device".
- **Rooms**: joined rooms with a search box, newest activity first, and a
  rail of joined Spaces ("Home" plus one chip per space) that filters the
  list to that space's rooms.
- **Lobby**: a space's full directory from `/hierarchy`, where rooms you
  haven't joined yet can be joined.
- **Room**: the live timeline with day separators, sender avatars, inline
  image previews, older history on scroll, a search box over loaded
  messages, "Jump to present", and the composer.
- **Settings**: account details, System/Light/Dark theme, sign out.

On a wide window the room list stays on the left and the room, lobby or
settings open beside it; on a narrow one (phones) they replace each other.

## Running

Everything the build needs except Flutter itself comes from
[pixi](https://pixi.sh) (`pixi.toml`): clang, CMake, Ninja,
pkg-config, GTK 3, sqlite, and Rust for
[`flutter_vodozemac`](https://pub.dev/packages/flutter_vodozemac) (end-to-end
encryption). conda-forge has no Flutter package, so install Flutter
separately and have `flutter` on your `PATH`.

conda-forge has no rustup either, which the encryption library's build
(cargokit) insists on, so `tool/bin/rustup` stands in for it inside the
environment: it reports pixi's Rust as the installed toolchain and runs
`cargo` directly.

```sh
cd flutter
pixi install
pixi run linux        # flutter run -d linux
```

flatkit is a git dependency in `pubspec.yaml`; `flutter pub upgrade flatkit`
picks up new commits. Other tasks:
`pixi run build-linux` and `pixi run test`. The environment's GTK is only
used to build: the install step rewrites the bundle's library search path
(with patchelf), so `build/linux/x64/release/bundle/neo` runs on the system's
GTK, fonts and GPU drivers, inside or outside pixi.

`pixi run linux` and `pixi run build-linux` first remove any Linux build
directory that CMake set up outside pixi (for example by an earlier plain
`flutter run`): CMake keeps the compiler and GTK it found first, and mixing
the system's with pixi's fails to link.

For Android, macOS, Windows or iOS, `flutter run` works as usual with that
platform's own toolchain.

## Tests

```sh
pixi run flutter analyze
pixi run test
```

## How it fits together

- `lib/src/session.dart`: the `Client`, sign-in/sign-out and session
  restore. A remembered session is a sqlite database under the app support
  directory (`~/.local/share/org.neo.neo/neo.sqlite` on Linux), holding the
  access token and the encryption keys, so the next launch reuses the same
  device instead of logging in again. Without "remember me" the database is
  in memory and gone when neo quits.
- `lib/src/sso.dart`: SSO through a short-lived localhost server that the
  homeserver redirects back to with a login token.
- `lib/src/rooms.dart`, `lib/src/timeline.dart`: room filtering, the lobby
  fetch and the timeline rows, free of widgets and covered by tests.
- `lib/src/media.dart`: avatars and image previews, fetched with the access
  token (authenticated media) and decrypted for encrypted rooms.
- `lib/src/ui/`: one file per screen, plus `shell.dart` for navigation and
  theming.

Unlike the Rust app, the Dart SDK runs a continuous `/sync` loop, so new
messages arrive as soon as the server sends them.

## Licence note

The Dart `matrix` SDK is AGPL-3.0, so a neo build that bundles it is
covered by the AGPL when distributed.
