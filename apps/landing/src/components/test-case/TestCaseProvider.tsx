'use client'

import { createContext, useContext, useState } from 'react'

import { TestStatusMap } from '@/types'

export type TestCaseFilter =
  | 'korean'
  | 'math'
  | 'science'
  | 'music'
  | 'western'
  | 'foreign-language'
  | 'ipa'
  | 'corpus'

export type TestCaseOptions = {
  filters: TestCaseFilter[]
  failedOnly: boolean
  type: 'list' | 'table'
}

export type FilterMap = Record<TestCaseFilter, string[]>

const TestCaseContext = createContext<{
  testStatusMap: TestStatusMap
  filterMap: FilterMap
  options: TestCaseOptions
  onChangeOptions: (options: Partial<TestCaseOptions>) => void
} | null>(null)

export function useTestCase() {
  const context = useContext(TestCaseContext)
  if (!context) {
    throw new Error('useTestCase must be used within a TestCaseProvider')
  }
  return context
}

export function TestCaseProvider({
  testStatusMap,
  filterMap,
  children,
}: {
  testStatusMap: TestStatusMap
  filterMap: FilterMap
  children: React.ReactNode
}) {
  const [options, setOptions] = useState<TestCaseOptions>({
    filters: ['korean'],
    failedOnly: false,
    type: 'list',
  })
  const handleChangeOptions = (options: Partial<TestCaseOptions>) => {
    setOptions((prev) => ({ ...prev, ...options }))
  }

  return (
    <TestCaseContext.Provider
      value={{
        filterMap,
        onChangeOptions: handleChangeOptions,
        options,
        testStatusMap,
      }}
    >
      {children}
    </TestCaseContext.Provider>
  )
}
