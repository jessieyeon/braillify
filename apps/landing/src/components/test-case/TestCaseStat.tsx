import { Center, Flex, Text } from '@devup-ui/react'
import { ComponentProps } from 'react'

interface TestCaseStatProps extends ComponentProps<typeof Center<'div'>> {
  showTotal?: boolean
  total: number
  success: number
  fail: number
  worldTotal?: number
  worldFail?: number
}

export function TestCaseStat({
  showTotal = false,
  total,
  success,
  fail,
  worldTotal,
  worldFail,
  ...props
}: TestCaseStatProps) {
  const hasFail = fail > 0
  const braillifyPercent = Math.round((success / total) * 100)

  const hasWorld = worldTotal != null && worldTotal > 0
  const worldSuccess = hasWorld ? worldTotal - worldFail! : 0
  const worldPercent = hasWorld
    ? Math.round((worldSuccess / worldTotal) * 100)
    : 0

  return (
    <Flex alignItems="center" gap="8px" styleOrder={1}>
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
        <Center
          bg="#2B2B2B"
          borderRadius="10px"
          gap="10px"
          px="16px"
          py="10px"
        >
          <Text color="#888" typography="progress">
            점자세상
          </Text>
          {showTotal && (
            <Text color="#888" typography="progress">
              전체 {worldTotal.toLocaleString()}
            </Text>
          )}
          <Text color="#888" typography="progress">
            성공 {worldSuccess.toLocaleString()}
          </Text>
          <Text color="$error" typography="progress">
            실패 {worldFail!.toLocaleString()}
          </Text>
          <Text
            color={worldPercent < braillifyPercent ? '$error' : '$text'}
            typography="progress"
          >
            ({worldPercent}%)
          </Text>
        </Center>
      )}
    </Flex>
  )
}
