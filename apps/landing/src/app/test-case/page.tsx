import 'katex/dist/katex.min.css'

import { Box, css, Flex, Text, VStack } from '@devup-ui/react'
import { readFile } from 'fs/promises'
import { Metadata } from 'next'

import { FailedOnlyInput } from '@/components/test-case/FailedOnlyInput'
import { TestCaseFilter } from '@/components/test-case/filter/TestCaseFilter'
import { TestCaseList } from '@/components/test-case/list/TestCaseList'
import { TestCaseTable } from '@/components/test-case/table/TestCaseTable'
import { TestCaseDisplayBoundary } from '@/components/test-case/TestCaseDisplayBoundary'
import { TestCaseFilterContainer } from '@/components/test-case/TestCaseFilterContainer'
import { TestCaseProvider } from '@/components/test-case/TestCaseProvider'
import { TestCaseRuleContainer } from '@/components/test-case/TestCaseRuleContainer'
import { TestCaseStat } from '@/components/test-case/TestCaseStat'
import { TestCaseTypeToggle } from '@/components/test-case/TestCaseTypeToggle'
import { createFilterMap, TEST_CASE_FILTERS } from '@/constants'
import { TestStatusMap } from '@/types'

export const metadata: Metadata = {
  alternates: {
    canonical: '/test-case',
  },
}

export default async function TestCasePage() {
  const [testStatus, ruleMap] = await Promise.all([
    readFile('../../test_status.json', 'utf-8').then((data) =>
      JSON.parse(data),
    ) as Promise<TestStatusMap>,
    readFile('../../rule_map.json', 'utf-8').then((data) =>
      JSON.parse(data),
    ) as Promise<Record<string, { title: string; description: string }>>,
  ])

  // Dynamically create filter map based on rule_map keys
  const filterMap = createFilterMap(Object.keys(ruleMap))
  let totalTest = 0
  let totalFail = 0
  let totalWorldTest = 0
  let totalWorldFail = 0
  let totalJeomsarangTest = 0
  let totalJeomsarangFail = 0
  const cases = Object.entries(ruleMap).map(([key, value]) => {
    totalTest += testStatus[key][0]
    totalFail += testStatus[key][1]
    totalWorldTest += testStatus[key][2]
    totalWorldFail += testStatus[key][3]
    totalJeomsarangTest += testStatus[key][4]
    totalJeomsarangFail += testStatus[key][5]

    const isBut = value.title.includes('다만')

    return (
      <TestCaseDisplayBoundary
        key={key}
        option="failedOnly"
        value={testStatus[key][1]}
      >
        {/* @todo 필터 관련 JSON 및 상태 관련 처리 */}
        <TestCaseDisplayBoundary option="filters" value={key}>
          <TestCaseRuleContainer key={key} exception={isBut}>
            <VStack gap="20px">
              <Flex
                alignItems="center"
                gap="20px"
                justifyContent={['space-between', null, null, 'flex-start']}
              >
                <Text color="$title" typography="docsTitle">
                  {value.title}
                </Text>
                <TestCaseStat
                  fail={testStatus[key][1]}
                  jeomsarangFail={testStatus[key][5]}
                  jeomsarangTotal={testStatus[key][4]}
                  success={testStatus[key][0] - testStatus[key][1]}
                  total={testStatus[key][0]}
                  worldFail={testStatus[key][3]}
                  worldTotal={testStatus[key][2]}
                />
              </Flex>
              <Text color="$text" typography="body" wordBreak="keep-all">
                {value.description}
              </Text>
            </VStack>
            <TestCaseDisplayBoundary option="type" value="table">
              <TestCaseTable results={testStatus[key][6]} />
            </TestCaseDisplayBoundary>
            <TestCaseDisplayBoundary option="type" value="list">
              <TestCaseList results={testStatus[key][6]} />
            </TestCaseDisplayBoundary>
          </TestCaseRuleContainer>
        </TestCaseDisplayBoundary>
      </TestCaseDisplayBoundary>
    )
  })

  return (
    <TestCaseProvider filterMap={filterMap} testStatusMap={testStatus}>
      <Box maxW="1520px" mx="auto" pb="40px" w="100%">
        <VStack
          gap="20px"
          px={['16px', null, null, '60px']}
          py={['30px', null, null, '40px']}
        >
          <VStack
            alignItems={['flex-start', null, null, 'center']}
            flexDir={[null, null, null, 'row']}
            gap={['10px', null, null, '20px']}
          >
            <Text color="$title" typography="title">
              테스트 케이스
            </Text>
            <TestCaseStat
              fail={totalFail}
              jeomsarangFail={totalJeomsarangFail}
              jeomsarangTotal={totalJeomsarangTest}
              showTotal
              success={totalTest - totalFail}
              total={totalTest}
              worldFail={totalWorldFail}
              worldTotal={totalWorldTest}
            />
          </VStack>
          <Text color="$text" typography="body" wordBreak="keep-all">
            모든 테스트 케이스는{' '}
            <Text
              _hover={{
                textDecoration: 'underline',
              }}
              as="a"
              color="$link"
              href="/2024 개정 한국 점자 규정.pdf"
              target="_blank"
            >
              2024 개정 한국 점자 규정
            </Text>
            을 기반으로 작성되었습니다.
          </Text>
        </VStack>
        <TestCaseFilterContainer>
          <VStack
            alignItems={['flex-end', null, null, 'center']}
            flexDir={['column-reverse', null, null, 'row']}
            gap="12px"
            justifyContent={[null, null, null, 'space-between']}
          >
            <Flex
              gap="10px"
              overflowX="auto"
              overflowY="visible"
              pb="2px"
              scrollbarWidth="none"
              w="100%"
            >
              {TEST_CASE_FILTERS.map((filter) => (
                <TestCaseFilter key={filter.value} value={filter.value}>
                  {filter.label}
                </TestCaseFilter>
              ))}
            </Flex>
            <Flex
              alignItems="center"
              color="$primary"
              gap="10px"
              typography="body"
              whiteSpace="nowrap"
            >
              <Text>목록 형식</Text>
              <TestCaseTypeToggle />
              <Text>표 형식</Text>
            </Flex>
          </VStack>
          <Flex alignItems="center" gap="10px">
            <FailedOnlyInput
              className={css({
                accentColor: '$primary',
                cursor: 'pointer',
                boxSize: '18px',
              })}
              id="failed-only"
              name="failed-only"
              type="checkbox"
            />
            <Text
              as="label"
              color="$primary"
              cursor="pointer"
              htmlFor="failed-only"
              typography="body"
            >
              실패한 케이스만 표시하기
            </Text>
          </Flex>
          <Box bg="$text" h="1px" />
        </TestCaseFilterContainer>
        {cases}
        <Box bg="$text" h="1px" mx={['16px', null, null, '60px']} />
      </Box>
    </TestCaseProvider>
  )
}
