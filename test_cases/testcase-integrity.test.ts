import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync } from "fs";
import { join } from "path";

/**
 * Braille internal notation → expected (index string) & unicode conversion.
 *
 * This mirrors the logic in braillove-case-collector/converter.py.
 * Each character in the `internal` field maps to a braille cell index (0–63).
 * `expected` is the concatenation of those indices as strings.
 * `unicode` is the concatenation of the corresponding braille Unicode characters.
 */

const PATTERN =
  " a1b'k2l@cif/msp\"e3h9o6r^djg>ntq,*5<-u8v.%[$+x!&;:4\\0z7(_?w]#y)=";
const BRAILLE =
  "⠀⠁⠂⠃⠄⠅⠆⠇⠈⠉⠊⠋⠌⠍⠎⠏⠐⠑⠒⠓⠔⠕⠖⠗⠘⠙⠚⠛⠜⠝⠞⠟⠠⠡⠢⠣⠤⠥⠦⠧⠨⠩⠪⠫⠬⠭⠮⠯⠰⠱⠲⠳⠴⠵⠶⠷⠸⠹⠺⠻⠼⠽⠾⠿";

const SPECIAL: Record<string, number> = {
  "{": 42,
  "}": 59,
  "~": 24,
  "`": 0,
  "|": 51,
};

function convert(internal: string): { expected: string; unicode: string } {
  let expected = "";
  let unicode = "";
  for (const ch of internal) {
    let idx: number;
    if (ch in SPECIAL) {
      idx = SPECIAL[ch];
    } else {
      idx = PATTERN.indexOf(ch);
      if (idx === -1) {
        throw new Error(
          `Character '${ch}' (U+${ch.charCodeAt(0).toString(16).padStart(4, "0")}) not found in pattern`,
        );
      }
    }
    expected += idx.toString();
    unicode += BRAILLE[idx];
  }
  return { expected, unicode };
}

/** Returns true if every character in `internal` is convertible by the basic pattern. */
function isConvertible(internal: string): boolean {
  for (const ch of internal) {
    if (ch in SPECIAL) continue;
    if (PATTERN.indexOf(ch) !== -1) continue;
    return false;
  }
  return true;
}

interface TestEntry {
  input: string;
  internal: string;
  expected: string;
  unicode: string;
}

function loadTestCases(dir: string): { file: string; entries: TestEntry[] }[] {
  const dirPath = join(import.meta.dir, dir);
  const files = readdirSync(dirPath).filter((f) => f.endsWith(".json")).sort();
  return files.map((file) => {
    const content = readFileSync(join(dirPath, file), "utf-8");
    return { file, entries: JSON.parse(content) as TestEntry[] };
  });
}

function runIntegrityTests(dir: string, label: string) {
  const testFiles = loadTestCases(dir);

  describe(`${label} test case integrity`, () => {
    for (const { file, entries } of testFiles) {
      describe(file, () => {
        for (let i = 0; i < entries.length; i++) {
          const entry = entries[i];
          const inputPreview =
            entry.input.length > 30
              ? entry.input.slice(0, 30) + "…"
              : entry.input;

          test(`[${i}] "${inputPreview}" has non-empty internal`, () => {
            expect(entry.internal).not.toBe("");
          });

          if (!entry.internal) continue;

          // Skip entries that use extended characters (uppercase math vars,
          // old Korean jamo, etc.) not covered by the basic 64-cell pattern.
          if (!isConvertible(entry.internal)) continue;

          test(`[${i}] "${inputPreview}" expected matches internal`, () => {
            const { expected } = convert(entry.internal);
            expect(expected).toBe(entry.expected);
          });

          test(`[${i}] "${inputPreview}" unicode matches internal`, () => {
            const { unicode } = convert(entry.internal);
            expect(unicode).toBe(entry.unicode);
          });
        }
      });
    }
  });
}

runIntegrityTests("korean", "Korean");
runIntegrityTests("math", "Math");
