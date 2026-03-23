import time
import os
import glob
import json
from pywinauto.application import Application

pattern = " a1b'k2l`cif/msp\"e3h9o6r^djg>ntq,*5<-u8v.%[$+x!&;:4\\0z7(_?w]#y)="
braille = "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿"


def raw_to_unicode_braille(raw_text: str) -> str:
    """Convert 점사랑 raw output to unicode braille."""
    result = ""
    for ch in raw_text:
        if ch in pattern:
            result += braille[pattern.index(ch)]
        elif ch == "@":
            result += braille[8]
        elif ch == "|":
            result += braille[51]
        else:
            raise ValueError(f"Unknown character in output: {repr(ch)}")
    return result


def main():
    app = None
    try:
        app = Application(backend="uia").start(
            r"C:\Program Files (x86)\Jeomsarang6\BrailleLove.exe"
        )
        print("BrailleLove started.")

        main_window = app.window(title="점사랑 6.0")
        main_window.set_focus()
        main_window.maximize()

        main_window.child_window(title="새문서", control_type="Button").click()
        main_window.child_window(title="확인(O)", control_type="Button").click()

        main_window = app.window(title=app.windows()[0].window_text())
        pane = main_window.child_window(control_type="Pane", title="작업 영역")
        output_edit = main_window.child_window(control_type="Edit", title="")

        test_case_files = sorted(glob.glob("../test_cases/*.json"))
        if not test_case_files:
            print("No test case files found in ../test_cases/")
            return

        total_cases = 0
        passed_cases = 0
        failed_cases = 0
        errors = 0
        failures_detail = []
        results_per_file = {}

        for test_file in test_case_files:
            file_name = os.path.basename(test_file)
            print(f"\n--- {file_name} ---")
            file_total = 0
            file_passed = 0

            with open(test_file, "r", encoding="utf-8") as f:
                records = json.load(f)
                for row in records:
                    korean_input = row["input"].strip()
                    expected_unicode = row["unicode"].strip()

                    if not korean_input or not expected_unicode:
                        continue

                    file_total += 1
                    total_cases += 1

                    try:
                        # Type input into 점사랑
                        time.sleep(0.3)
                        pane.type_keys(
                            korean_input.replace(" ", "{SPACE}")
                            .replace("(", "{(}")
                            .replace(")", "{)}"),
                            pause=0.05,
                        )
                        time.sleep(0.3)

                        # Read output
                        raw_output = output_edit.get_value()
                        actual_unicode = raw_to_unicode_braille(raw_output)

                        if actual_unicode == expected_unicode:
                            passed_cases += 1
                            file_passed += 1
                        else:
                            failed_cases += 1
                            detail = {
                                "file": file_name,
                                "input": korean_input,
                                "expected": expected_unicode,
                                "actual": actual_unicode,
                            }
                            failures_detail.append(detail)
                            print(
                                f"  FAIL: '{korean_input}' expected={expected_unicode} actual={actual_unicode}"
                            )

                        # Clear input
                        main_window.set_focus()
                        time.sleep(0.3)
                        pane.type_keys("{BACKSPACE}" * len(korean_input))
                        while output_edit.get_value() != "":
                            pane.type_keys("{BACKSPACE}")

                    except Exception as e:
                        errors += 1
                        print(f"  ERROR: '{korean_input}' -> {e}")
                        # Try to clear
                        try:
                            main_window.set_focus()
                            pane.type_keys("^a{DELETE}")
                            time.sleep(0.5)
                        except:
                            pass

            results_per_file[file_name] = {
                "total": file_total,
                "passed": file_passed,
                "failed": file_total - file_passed,
            }
            if file_total > 0:
                pct = file_passed / file_total * 100
                print(f"  {file_passed}/{file_total} passed ({pct:.1f}%)")

        # Summary
        print("\n" + "=" * 60)
        print("VERIFICATION SUMMARY")
        print("=" * 60)
        print(f"Total cases:  {total_cases}")
        print(f"Passed:       {passed_cases}")
        print(f"Failed:       {failed_cases}")
        print(f"Errors:       {errors}")
        if total_cases > 0:
            accuracy = passed_cases / total_cases * 100
            print(f"Accuracy:     {accuracy:.2f}%")
        print("=" * 60)

        # Per-file breakdown
        print("\nPer-file results:")
        for fname, stats in sorted(results_per_file.items()):
            pct = stats["passed"] / stats["total"] * 100 if stats["total"] > 0 else 0
            status = "PASS" if stats["failed"] == 0 else "FAIL"
            print(
                f"  [{status}] {fname}: {stats['passed']}/{stats['total']} ({pct:.1f}%)"
            )

        # Save results to JSON
        report = {
            "total": total_cases,
            "passed": passed_cases,
            "failed": failed_cases,
            "errors": errors,
            "accuracy_percent": round(passed_cases / total_cases * 100, 2)
            if total_cases > 0
            else 0,
            "per_file": results_per_file,
            "failures": failures_detail[:100],  # Cap at 100 for readability
        }
        with open("../jeomsarang_verify_result.json", "w", encoding="utf-8") as f:
            json.dump(report, f, ensure_ascii=False, indent=2)
        print(f"\nDetailed results saved to jeomsarang_verify_result.json")

    except Exception as e:
        print(f"Error: {e}")
        import traceback

        traceback.print_exc()
    finally:
        if app:
            app.kill()
            print("BrailleLove terminated.")


if __name__ == "__main__":
    main()
