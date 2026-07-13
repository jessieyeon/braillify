import { spawnSync } from "node:child_process";

// cargo-tarpaulin's llvm coverage engine is only deterministic on Linux.
// On Windows the same engine miscounts a shifting ~15-line set on every run
// (an observer defect, not a real gap: the encoder has no OS-specific
// production branches, and Linux/macOS report a stable 100%). macOS matches
// Linux but we keep a single authoritative platform to avoid redundant uploads.
//
// So Linux is the authoritative coverage platform and enforces `--fail-under
// 100`; every other platform just runs the same tests for pass/fail.
const measureCoverage = process.platform === "linux";

const cargoArgs = measureCoverage
  ? [
      "tarpaulin",
      "--engine",
      "llvm",
      "--out",
      "xml",
      "--out",
      "stdout",
      "--timeout",
      "300",
      "--fail-under",
      "100",
      "--",
      "--skip",
      "test_by_testcase",
    ]
  : ["test", "-p", "braillify", "--", "--skip", "test_by_testcase"];

const result = spawnSync("cargo", cargoArgs, { stdio: "inherit" });
if (result.error) {
  throw result.error;
}
process.exit(result.status ?? 1);
