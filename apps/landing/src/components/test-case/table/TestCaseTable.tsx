import { css, Flex, Image, VStack } from '@devup-ui/react'
import { Text } from '@devup-ui/react'

import { Table, Tbody, Td, Th, Thead, Tr } from '@/components/test-case/table'
import { TestStatus } from '@/types'

import { TestCaseDisplayBoundary } from '../TestCaseDisplayBoundary'

export function TestCaseTable({ results }: { results: TestStatus[4] }) {
  return (
    <Table>
      <Thead
        className={css({ display: ['none', null, null, 'table-header-group'] })}
      >
        <Tr>
          <Th>번호</Th>
          <Th>예문</Th>
          <Th>정답</Th>
          <Th>결과</Th>
          <Th>성공 여부</Th>
          <Th>점자세상</Th>
        </Tr>
      </Thead>
      <Tbody>
        {results.map(
          (
            [text, note, expected, actual, isSuccess, world, worldIsSuccess],
            index,
          ) => (
            <TestCaseDisplayBoundary
              key={index}
              option="failedOnly"
              value={Number(!isSuccess)}
            >
              <Tr
                key={index + 'desktop'}
                className={css({
                  bg: isSuccess ? 'unset' : '#D8D8D8',
                  display: ['none', null, null, 'table-row'],
                })}
                data-responsive="desktop"
              >
                <Td>{index + 1}</Td>
                <Td>
                  {text}
                  {note ? ` (${note})` : null}
                </Td>
                <Td>{expected}</Td>
                <Td>{actual}</Td>
                <Td
                  className={css({
                    color: isSuccess ? '$success' : '$error',
                    textAlign: 'center',
                  })}
                >
                  <Flex alignItems="center" gap="4px">
                    <Text whiteSpace="nowrap">
                      {isSuccess ? '성공' : '실패'}
                    </Text>
                    <Image
                      alt={isSuccess ? 'success' : 'error'}
                      boxSize="24px"
                      src={
                        isSuccess
                          ? '/images/test-case/success.svg'
                          : '/images/test-case/error.svg'
                      }
                    />
                  </Flex>
                </Td>
                <Td>
                  {world ? (
                    <Flex alignItems="center" gap="4px">
                      <Text
                        className={css({
                          color: worldIsSuccess ? '$success' : '$error',
                        })}
                        whiteSpace="nowrap"
                      >
                        {worldIsSuccess ? '일치' : '불일치'}
                      </Text>
                    </Flex>
                  ) : (
                    <Text color="#666">-</Text>
                  )}
                </Td>
              </Tr>
              <Tr
                key={index + 'mobile'}
                className={css({
                  bg: isSuccess ? 'unset' : '#D8D8D8',
                  display: ['table-row', null, null, 'none'],
                })}
                data-responsive="mobile"
              >
                <Td className={css({ pb: '16px', pt: '10px' })}>
                  <VStack gap="8px">
                    <Flex
                      alignItems="center"
                      gap="4px"
                      justifyContent="space-between"
                      px="10px"
                    >
                      <Text>{index + 1}</Text>
                      <Image
                        alt={isSuccess ? 'success' : 'error'}
                        boxSize="24px"
                        src={
                          isSuccess
                            ? '/images/test-case/success.svg'
                            : '/images/test-case/error.svg'
                        }
                      />
                    </Flex>
                    <Flex alignItems="center" gap="10px" px="10px">
                      <Text typography="bodyBold">예문</Text>
                      <Text>
                        {text}
                        {note ? ` (${note})` : null}
                      </Text>
                    </Flex>
                    <Flex alignItems="center" gap="10px" px="10px">
                      <Text typography="bodyBold">정답</Text>
                      <Text>{expected}</Text>
                    </Flex>
                    <Flex alignItems="center" gap="10px" px="10px">
                      <Text typography="bodyBold">결과</Text>
                      <Text>{actual}</Text>
                    </Flex>
                    {world ? (
                      <Flex alignItems="center" gap="10px" px="10px">
                        <Text typography="bodyBold">점자세상</Text>
                        <Text
                          className={css({
                            color: worldIsSuccess ? '$success' : '$error',
                          })}
                        >
                          {worldIsSuccess ? '일치' : '불일치'} ({world})
                        </Text>
                      </Flex>
                    ) : null}
                  </VStack>
                </Td>
              </Tr>
            </TestCaseDisplayBoundary>
          ),
        )}
      </Tbody>
    </Table>
  )
}
