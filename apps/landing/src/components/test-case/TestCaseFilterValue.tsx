'use client'

import { TestCaseFilter, useTestCase } from './TestCaseProvider'

export function TestCaseFilterValue({
  map,
}: {
  map: Record<TestCaseFilter, string>
}) {
  const { options } = useTestCase()
  const selected = options.filters[0]
  return map[selected]
}
