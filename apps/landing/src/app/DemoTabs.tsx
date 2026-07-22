'use client'
import { Flex, Text, VStack } from '@devup-ui/react'
import { useState } from 'react'

import { MathTrans } from './MathTrans'
import { Trans } from './Trans'

const TABS = [
  { key: 'korean', label: '한글' },
  { key: 'math', label: '수학' },
] as const

type DemoMode = (typeof TABS)[number]['key']

export function DemoTabs() {
  const [mode, setMode] = useState<DemoMode>('korean')
  return (
    <VStack flex="1" gap={['16px', null, null, '30px']} w="100%">
      <Flex
        aria-label="점역 데모 종류"
        gap="10px"
        justifyContent={['center', null, null, 'flex-start']}
        role="tablist"
      >
        {TABS.map(({ key, label }) => (
          <Text
            key={key}
            aria-selected={mode === key}
            as="button"
            bg={mode === key ? '$text' : 'transparent'}
            border="1px solid $text"
            borderRadius="9999px"
            color={mode === key ? '$background' : '$text'}
            cursor="pointer"
            onClick={() => setMode(key)}
            px={['20px', null, null, '28px']}
            py={['8px', null, null, '10px']}
            role="tab"
            typography="button"
          >
            {label}
          </Text>
        ))}
      </Flex>
      {mode === 'korean' ? <Trans /> : <MathTrans />}
    </VStack>
  )
}
