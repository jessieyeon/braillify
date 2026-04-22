'use client'

import { useTestCase } from './TestCaseProvider'
import { TestCaseStat } from './TestCaseStat'

export function TestCaseStatFiltered() {
  const { options, filterTotalMap } = useTestCase()
  const selected = options.filters[0]
  return (
    <TestCaseStat
      fail={filterTotalMap[selected].braillify.fail}
      jeomsarangFail={filterTotalMap[selected].jeomsarang.fail}
      jeomsarangTotal={filterTotalMap[selected].jeomsarang.total}
      showTotal
      success={
        filterTotalMap[selected].braillify.total -
        filterTotalMap[selected].braillify.fail
      }
      total={filterTotalMap[selected].braillify.total}
      worldFail={filterTotalMap[selected].world.fail}
      worldTotal={filterTotalMap[selected].world.total}
    />
  )
}
