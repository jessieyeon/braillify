/**
 * Fetch braille conversion results from 점자세상 (braillekorea.org) API
 * and add "world" field to each test case entry.
 *
 * Usage: bun run scripts/fetch-world.ts
 *
 * Skips:
 *  - Entries with note="LaTeX" (pure LaTeX duplicates)
 *  - Entries with empty input
 */

import { readdir, readFile, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

const API_URL = 'https://www.braillekorea.org/lecture/braille_proc.asp'
const DELAY_MS = 100
const TEST_CASES_DIR = join(import.meta.dirname!, '..', 'test_cases')

interface TestCaseEntry {
  input: string
  internal: string
  expected: string
  unicode: string
  note?: string
  world?: string
}

async function fetchBraille(input: string): Promise<string> {
  const body = `source_text=${encodeURIComponent(input)}`
  const res = await fetch(API_URL, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/x-www-form-urlencoded; charset=UTF-8',
      'X-Requested-With': 'XMLHttpRequest',
      Accept: 'application/json, text/javascript, */*; q=0.01',
      Referer: 'https://www.braillekorea.org/lecture/braille.asp',
    },
    body,
  })

  if (!res.ok) {
    throw new Error(`API returned ${res.status}`)
  }

  const data = (await res.json()) as {
    ascii: string
    braille: string
    count: number
  }
  return data.braille
}

function shouldSkip(entry: TestCaseEntry): boolean {
  if (entry.note === 'LaTeX') return true
  if (!entry.input || entry.input.trim() === '') return true
  return false
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

async function processFile(
  filePath: string,
): Promise<{
  total: number
  fetched: number
  skipped: number
  errors: number
}> {
  const raw = await readFile(filePath, 'utf-8')
  const entries: TestCaseEntry[] = JSON.parse(raw)
  let fetched = 0
  let skipped = 0
  let errors = 0

  for (const entry of entries) {
    if (shouldSkip(entry)) {
      entry.world = ''
      skipped++
      continue
    }

    try {
      entry.world = await fetchBraille(entry.input)
      fetched++
    } catch (err) {
      entry.world = ''
      errors++
      console.error(`  Error for "${entry.input.slice(0, 30)}...": ${err}`)
    }

    await sleep(DELAY_MS)
  }

  await writeFile(filePath, JSON.stringify(entries, null, 2) + '\n', 'utf-8')
  return { total: entries.length, fetched, skipped, errors }
}

async function main() {
  const dirs = await readdir(TEST_CASES_DIR)
  let grandTotal = 0
  let grandFetched = 0
  let grandSkipped = 0
  let grandErrors = 0

  for (const dir of dirs) {
    const dirPath = join(TEST_CASES_DIR, dir)
    const stat = await Bun.file(dirPath).exists()
    // skip non-directory entries (like .test.ts files)
    try {
      const files = await readdir(dirPath)
      const jsonFiles = files.filter((f) => f.endsWith('.json'))
      if (jsonFiles.length === 0) continue

      console.log(`\n📁 ${dir}/`)

      for (const file of jsonFiles) {
        const filePath = join(dirPath, file)
        process.stdout.write(`  ${file} ... `)
        const stats = await processFile(filePath)
        console.log(
          `✓ ${stats.fetched} fetched, ${stats.skipped} skipped, ${stats.errors} errors (${stats.total} total)`,
        )
        grandTotal += stats.total
        grandFetched += stats.fetched
        grandSkipped += stats.skipped
        grandErrors += stats.errors
      }
    } catch {
      // not a directory, skip
      continue
    }
  }

  console.log(`\n${'='.repeat(60)}`)
  console.log(`Total: ${grandTotal} entries`)
  console.log(
    `Fetched: ${grandFetched} | Skipped: ${grandSkipped} | Errors: ${grandErrors}`,
  )
  console.log(`${'='.repeat(60)}`)
}

main().catch(console.error)
