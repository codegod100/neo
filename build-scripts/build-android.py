#!/usr/bin/env python3
"""Build Neo's APK and add permissions Pixiewood cannot declare itself."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
from pathlib import Path


def run(*args: str) -> None:
    subprocess.run(args, check=True)


def pixiewood_command(explicit: str | None, root: Path) -> Path:
    candidates = [
        explicit,
        os.environ.get("PIXIEWOOD"),
        shutil.which("pixiewood"),
        str(root.parent / "gtk-android-builder/pixiewood"),
    ]
    for candidate in candidates:
        if not candidate:
            continue
        command = Path(candidate).expanduser().resolve()
        if command.is_file() and os.access(command, os.X_OK):
            return command

    raise SystemExit(
        "error: Pixiewood not found. Ensure tools/pixiewood is executable, "
        "set PIXIEWOOD, or pass --pixiewood /path/to/pixiewood"
    )


def android_home(explicit: str | None) -> Path:
    candidates = [
        explicit,
        os.environ.get("ANDROID_HOME"),
        os.environ.get("ANDROID_SDK_ROOT"),
    ]
    adb = shutil.which("adb")
    if adb:
        candidates.append(str(Path(adb).resolve().parent.parent))
    candidates.append(str(Path.home() / "Android/Sdk"))
    candidates.append("/opt/android-sdk")

    for candidate in candidates:
        if not candidate:
            continue
        sdk = Path(candidate).expanduser().resolve()
        if (sdk / "platform-tools/adb").is_file():
            return sdk

    raise SystemExit(
        "error: Android SDK not found. Set ANDROID_HOME or ANDROID_SDK_ROOT, "
        "put adb on PATH, or pass --android-home /path/to/sdk"
    )


def built_apk(root: Path) -> Path:
    output_dir = root / ".pixiewood/android/app/build/outputs/apk"
    preferred = output_dir / "debug/app-debug.apk"
    if preferred.is_file():
        return preferred

    apks = sorted(output_dir.glob("**/*.apk"))
    if len(apks) != 1:
        found = ", ".join(str(path) for path in apks) or "none"
        raise RuntimeError(
            f"expected one APK beneath {output_dir}, found: {found}"
        )
    return apks[0]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--android-home",
        help="Android SDK root (otherwise discovered from the environment)",
    )
    parser.add_argument(
        "--pixiewood",
        help="path to Pixiewood (otherwise discovered from the environment)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="copy the generated APK to this Buck output path",
    )
    parser.add_argument(
        "--deploy",
        action="store_true",
        help="install the built APK on the attached Android device",
    )
    args = parser.parse_args()

    root = Path.cwd()
    if not (root / "pixiewood.xml").is_file():
        raise SystemExit(
            "error: run this command from the Neo repository root"
        )
    sdk = android_home(args.android_home)
    pixiewood = pixiewood_command(args.pixiewood, root)
    manifest = root / ".pixiewood/android/app/src/main/AndroidManifest.xml"
    activity = root / (
        ".pixiewood/android/app/src/main/java/"
        "org/gtk/android/ToplevelActivity.java"
    )
    im_context = root / (
        ".pixiewood/android/app/src/main/java/org/gtk/android/ImContext.java"
    )

    run(str(pixiewood), "prepare", "-s", str(sdk), "pixiewood.xml")
    run(str(pixiewood), "generate")

    text = manifest.read_text()
    internet = (
        '  <uses-permission android:name="android.permission.INTERNET"/>\n'
    )
    if "android.permission.INTERNET" not in text:
        marker = (
            '  <uses-permission '
            'android:name="android.permission.REORDER_TASKS"/>'
        )
        if marker not in text:
            raise RuntimeError("Pixiewood manifest permission marker is missing")
        manifest.write_text(text.replace(marker, internet + marker))

    java = activity.read_text()
    if "openExternalUri" not in java:
        class_marker = "public class ToplevelActivity extends Activity {\n"
        helper = """public class ToplevelActivity extends Activity {
        private static ToplevelActivity latestActivity;

        public static void openExternalUri(String uri) {
                android.util.Log.i("Neo", "openExternalUri: " + uri);
                ToplevelActivity activity = latestActivity;
                if (activity == null)
                        throw new IllegalStateException("No active GTK activity");
                activity.runOnUiThread(() -> {
                        Intent intent = new Intent(Intent.ACTION_VIEW, Uri.parse(uri));
                        activity.startActivity(intent);
                });
        }
"""
        create_marker = "                super.onCreate(savedInstanceState);\n"
        if class_marker not in java or create_marker not in java:
            raise RuntimeError("Pixiewood ToplevelActivity layout changed")
        java = java.replace(class_marker, helper, 1)
        java = java.replace(
            create_marker,
            create_marker + "                latestActivity = this;\n",
            1,
        )
        activity.write_text(java)
    elif "latestActivity = this;" not in java:
        create_marker = "                super.onCreate(savedInstanceState);\n"
        if create_marker not in java:
            raise RuntimeError("Pixiewood activity creation hook is missing")
        activity.write_text(
            java.replace(
                create_marker,
                create_marker + "                latestActivity = this;\n",
                1,
            )
        )

    java = im_context.read_text()
    old_delete = "\t\t\tfor (int i = 0; i < leftLength; i++) sendKeyEvent(deleteKey);"
    new_delete = """\t\t\tfor (int i = 0; i < leftLength; i++) {
\t\t\t\tsendKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL));
\t\t\t\tsendKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_DEL));
\t\t\t}"""
    if old_delete in java:
        java = java.replace(old_delete, new_delete, 1)
        java = java.replace(
            "\t\t\tKeyEvent deleteKey = new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL);\n",
            "",
            1,
        )
        im_context.write_text(java)

    java = im_context.read_text()
    delete_marker = """\t\t@Override
\t\tpublic boolean deleteSurroundingText(int leftLength, int rightLength) {
"""
    send_key = """\t\t@Override
\t\tpublic boolean sendKeyEvent(KeyEvent event) {
\t\t\tandroid.util.Log.i("Neo", "IME key: code=" + event.getKeyCode()
\t\t\t\t+ " action=" + event.getAction() + " repeat=" + event.getRepeatCount());
\t\t\tif (event.getKeyCode() == KeyEvent.KEYCODE_DEL
\t\t\t\t\t&& event.getAction() == KeyEvent.ACTION_DOWN
\t\t\t\t\t&& event.getRepeatCount() > 0) {
\t\t\t\tfor (int i = 0; i < 256; i++) {
\t\t\t\t\tsuper.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL));
\t\t\t\t\tsuper.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_DEL));
\t\t\t\t}
\t\t\t\treturn true;
\t\t\t}
\t\t\treturn super.sendKeyEvent(event);
\t\t}

""" + delete_marker
    if 'android.util.Log.i("Neo", "IME key:' not in java:
        if delete_marker not in java:
            raise RuntimeError("Pixiewood IME deletion hook is missing")
        im_context.write_text(java.replace(delete_marker, send_key, 1))

    activity_java = activity.read_text()
    key_marker = """\t\t\t@Override
\t\t\tpublic boolean onKeyUp(int keyCode, KeyEvent event) {
\t\t\t\treturn keyEventProxy(event);
\t\t\t}
"""
    long_press = key_marker + """
                        @Override
                        public boolean onKeyLongPress(int keyCode, KeyEvent event) {
                                if (keyCode != KeyEvent.KEYCODE_DEL)
                                        return super.onKeyLongPress(keyCode, event);
                                for (int i = 0; i < 256; i++) {
                                        keyEventProxy(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL));
                                        keyEventProxy(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_DEL));
                                }
                                return true;
                        }
"""
    if "onKeyLongPress(int keyCode" not in activity_java:
        if key_marker not in activity_java:
            raise RuntimeError("Pixiewood key handler layout changed")
        activity.write_text(activity_java.replace(key_marker, long_press, 1))

    activity_java = activity.read_text()
    key_down = """\t\t\tpublic boolean onKeyDown(int keyCode, KeyEvent event) {
\t\t\t\treturn keyEventProxy(event);
\t\t\t}"""
    repeat_delete = """\t\t\tpublic boolean onKeyDown(int keyCode, KeyEvent event) {
\t\t\t\tif (keyCode == KeyEvent.KEYCODE_DEL && event.getRepeatCount() > 0) {
\t\t\t\t\tfor (int i = 0; i < 256; i++) {
\t\t\t\t\t\tkeyEventProxy(new KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_DEL));
\t\t\t\t\t\tkeyEventProxy(new KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_DEL));
\t\t\t\t\t}
\t\t\t\t\treturn true;
\t\t\t\t}
\t\t\t\treturn keyEventProxy(event);
\t\t\t}"""
    if "event.getRepeatCount() > 0" not in activity_java:
        if key_down not in activity_java:
            raise RuntimeError("Pixiewood key-down handler layout changed")
        activity.write_text(activity_java.replace(key_down, repeat_delete, 1))

    run(str(pixiewood), "build")
    apk = built_apk(root)
    print(f"APK: {apk}")
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(apk, args.output)
    if args.deploy:
        adb = sdk / "platform-tools/adb"
        run(str(adb), "install", "-r", str(apk))


if __name__ == "__main__":
    main()
