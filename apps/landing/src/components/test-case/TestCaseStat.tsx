import { Center, Flex, Text } from '@devup-ui/react'
import { ComponentProps } from 'react'

interface CompetitorStatProps {
  label: string
  total: number
  fail: number
  braillifyPercent: number
  showTotal: boolean
}

function CompetitorStat({
  label,
  total,
  fail,
  braillifyPercent,
  showTotal,
}: CompetitorStatProps) {
  const success = total - fail
  const percent = Math.round((success / total) * 100)

  return (
    <Center bg="#2B2B2B" borderRadius="10px" gap="10px" px="16px" py="10px">
      <Text color="#888" typography="progress">
        {label}
      </Text>
      {showTotal && (
        <Text color="#888" typography="progress">
          전체 {total.toLocaleString()}
        </Text>
      )}
      <Text color="#888" typography="progress">
        성공 {success.toLocaleString()}
      </Text>
      <Text color="$error" typography="progress">
        실패 {fail.toLocaleString()}
      </Text>
      <Text
        color={percent < braillifyPercent ? '$error' : '$text'}
        typography="progress"
      >
        ({percent}%)
      </Text>
    </Center>
  )
}

interface TestCaseStatProps extends ComponentProps<typeof Center<'div'>> {
  showTotal?: boolean
  total: number
  success: number
  fail: number
  worldTotal?: number
  worldFail?: number
  jeomsarangTotal?: number
  jeomsarangFail?: number
}

export function TestCaseStat({
  showTotal = false,
  total,
  success,
  fail,
  worldTotal,
  worldFail,
  jeomsarangTotal,
  jeomsarangFail,
  ...props
}: TestCaseStatProps) {
  const hasFail = fail > 0
  const braillifyPercent = Math.round((success / total) * 100)

  const hasWorld = worldTotal != null && worldTotal > 0
  const hasJeomsarang = jeomsarangTotal != null && jeomsarangTotal > 0

  return (
    <Flex
      alignItems="center"
      flexWrap="wrap"
      gap="8px"
      styleOrder={1}
    >
      <Center
        bg="$menuHover"
        borderRadius="10px"
        gap="10px"
        px="16px"
        py="10px"
        {...props}
      >
        {showTotal && (
          <Text color="$text" typography="progress">
            전체 {total.toLocaleString()}
          </Text>
        )}
        <Text color="$text" typography="progress">
          성공 {success.toLocaleString()}
        </Text>
        <Text color={hasFail ? '$error' : '$text'} typography="progress">
          실패 {fail.toLocaleString()}
        </Text>
        <Text
          color={showTotal ? '$text' : hasFail ? '$error' : '$success'}
          typography="progress"
        >
          ({braillifyPercent}%)
        </Text>
      </Center>
      {hasWorld && (
        <CompetitorStat
          braillifyPercent={braillifyPercent}
          fail={worldFail!}
          label="점자세상"
          showTotal={showTotal}
          total={worldTotal}
        />
      )}
      {hasJeomsarang && (
        <CompetitorStat
          braillifyPercent={braillifyPercent}
          fail={jeomsarangFail!}
          label="점사랑"
          showTotal={showTotal}
          total={jeomsarangTotal}
        />
      )}
    </Flex>
  )
}
