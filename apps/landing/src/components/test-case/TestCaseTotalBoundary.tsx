'use client'

import { useTestCase } from './TestCaseProvider'

export function TestCaseTotalBoundary({
  reverse = false,
  children,
}: {
  reverse?: boolean
  children: React.ReactNode
}) {
  const { filterMap, options } = useTestCase()
  const selected = options.filters[0]
  const length = filterMap[selected].length
  if (reverse) return length ? null : children
  return length ? children : null
}
