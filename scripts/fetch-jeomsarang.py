"""
Fetch braille conversion results from 점사랑 6.0 (BrailleLove.exe)
and add "jeomsarang" field to each test case entry.

Usage:
  cd braillove-case-collector && uv run ../scripts/fetch-jeomsarang.py

Requires:
  - 점사랑 6.0 installed at C:\\Program Files (x86)\\Jeomsarang6\\BrailleLove.exe
  - pywinauto (installed via braillove-case-collector's uv env)

NOTE: This script takes over the active window. Run when PC is idle.
      ~2000 entries × ~1s each ≈ 30-35 minutes.
"""

import json
import os
import sys
import time
import glob

from pywinauto.application import Application

PATTERN = " a1b'k2l`cif/msp\"e3h9o6r^djg>ntq,*5<-u8v.%[$+x!&;:4\\0z7(_?w]#y)="
BRAILLE = "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿"

SPECIAL_MAP = {"@": 8, "|": 51}

TEST_CASES_DIR = os.path.join(os.path.dirname(__file__), "..", "test_cases")


def internal_to_unicode(internal: str) -> str:
    """Convert 점사랑 internal notation to unicode braille."""
    result = ""
    for ch in internal:
        if ch in PATTERN:
            result += BRAILLE[PATTERN.index(ch)]
        elif ch in SPECIAL_MAP:
            result += BRAILLE[SPECIAL_MAP[ch]]
        else:
            # Unknown character — skip gracefully
            return ""
    return result


def should_skip(entry: dict) -> bool:
    if entry.get("note") == "LaTeX":
        return True
    if not entry.get("input", "").strip():
        return True
    return False


def escape_for_typekeys(text: str) -> str:
    """Escape special characters for pywinauto type_keys."""
    return (
        text.replace("{", "{{}")
        .replace("}", "{}}")
        .replace("(", "{(}")
        .replace(")", "{)}")
        .replace("+", "{+}")
        .replace("^", "{^}")
        .replace("%", "{%}")
        .replace("~", "{~}")
        .replace(" ", "{SPACE}")
    )


def main():
    app = None
    try:
        print("Starting BrailleLove.exe...")
        app = Application(backend="uia").start(
            r"C:\Program Files (x86)\Jeomsarang6\BrailleLove.exe"
        )
        time.sleep(2)

        main_window = app.window(title="점사랑 6.0")
        main_window.set_focus()
        main_window.maximize()

        # New document
        main_window.child_window(title="새문서", control_type="Button").click()
        time.sleep(0.5)
        main_window.child_window(title="확인(O)", control_type="Button").click()
        time.sleep(0.5)

        main_window = app.window(title=app.windows()[0].window_text())
        pane = main_window.child_window(control_type="Pane", title="작업 영역")
        output = main_window.child_window(control_type="Edit", title="")

        # Find all test case JSON files
        json_files = []
        for subdir in sorted(os.listdir(TEST_CASES_DIR)):
            subdir_path = os.path.join(TEST_CASES_DIR, subdir)
            if not os.path.isdir(subdir_path):
                continue
            for f in sorted(os.listdir(subdir_path)):
                if f.endswith(".json"):
                    json_files.append(os.path.join(subdir_path, f))

        grand_total = 0
        grand_fetched = 0
        grand_skipped = 0
        grand_errors = 0

        for filepath in json_files:
            filename = os.path.basename(filepath)
            dirpart = os.path.basename(os.path.dirname(filepath))
            label = f"{dirpart}/{filename}"

            with open(filepath, "r", encoding="utf-8") as f:
                entries = json.load(f)

            fetched = 0
            skipped = 0
            errors = 0

            for entry in entries:
                if should_skip(entry):
                    entry["jeomsarang"] = ""
                    skipped += 1
                    continue

                text = entry["input"]
                try:
                    # Type the input
                    escaped = escape_for_typekeys(text)
                    pane.type_keys(escaped, pause=0.02)
                    time.sleep(0.3)

                    # Read the internal output
                    internal = output.get_value()
                    unicode_result = internal_to_unicode(internal)
                    entry["jeomsarang"] = unicode_result
                    fetched += 1

                    # Clear: Ctrl+A then Delete
                    pane.type_keys("^a{DELETE}")
                    time.sleep(0.1)

                except Exception as e:
                    entry["jeomsarang"] = ""
                    errors += 1
                    print(f"  Error for '{text[:30]}...': {e}")
                    try:
                        pane.type_keys("^a{DELETE}")
                        time.sleep(0.2)
                    except:
                        pass

            # Save after each file
            with open(filepath, "w", encoding="utf-8") as f:
                json.dump(entries, f, ensure_ascii=False, indent=2)
                f.write("\n")

            print(
                f"  {label} ... {fetched} fetched, {skipped} skipped, {errors} errors ({len(entries)} total)"
            )
            grand_total += len(entries)
            grand_fetched += fetched
            grand_skipped += skipped
            grand_errors += errors

        print(f"\n{'=' * 60}")
        print(f"Total: {grand_total} entries")
        print(
            f"Fetched: {grand_fetched} | Skipped: {grand_skipped} | Errors: {grand_errors}"
        )
        print(f"{'=' * 60}")

    except Exception as e:
        print(f"Fatal error: {e}")
        import traceback

        traceback.print_exc()
    finally:
        if app:
            try:
                app.kill()
                print("BrailleLove terminated.")
            except:
                pass


if __name__ == "__main__":
    main()
