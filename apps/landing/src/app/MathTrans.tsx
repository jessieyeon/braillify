'use client'
import { Box, Flex, Image, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

import { MathTransInput } from './MathTransInput'
import { TransInput } from './TransInput'

export function MathTrans() {
  const [latex, setLatex] = useState('')
  const [translateToUnicode, setTranslateToUnicode] = useState<
    (input: string) => string
  >(() => () => '')
  useEffect(() => {
    import('braillify').then((mod) => {
      setTranslateToUnicode(() => (input: string) => {
        try {
          return mod.translateToUnicode(input)
        } catch (e) {
          console.error(e)
          return '점역할 수 없는 수식이 포함되어 있습니다.'
        }
      })
    })
  }, [])

  const braille = latex.trim() ? translateToUnicode(`$${latex}$`) : ''

  return (
    <VStack flex="1" gap={['16px', null, null, '30px']}>
      <Flex
        alignItems="flex-start"
        gap={['10px', null, null, '20px']}
        justifyContent={['center', null, null, 'flex-start']}
      >
        <Box
          aria-hidden="true"
          bg="$text"
          flexShrink={0}
          h={['20px', null, null, '32px']}
          maskImage="url(/images/home/finger-point.svg)"
          maskPosition="center"
          maskRepeat="no-repeat"
          maskSize="contain"
          w={['17px', null, null, '28px']}
        />
        <Text color="$text" pos="relative" top="-2px" typography="mainTextSm">
          수식도 점자가 됩니다. 수식 키보드로 입력해 수학 점역을 체험해보세요!
        </Text>
      </Flex>
      <Flex
        alignItems="center"
        flexDirection={['column', null, null, 'row']}
        gap={['12px', null, null, '30px']}
        h={['auto', null, null, '500px']}
        w="100%"
      >
        <MathTransInput
          onLatexChange={setLatex}
          placeholder="수식을 입력하면 이곳에 수학 점자가 표시됩니다."
        />
        <Flex aria-hidden="true">
          <Image
            alt=""
            display={['none', null, null, 'block']}
            mr="10px"
            role="presentation"
            src="/images/home/translate-arrow-circle.svg"
            w="16px"
          />
          <Image
            alt=""
            role="presentation"
            src="/images/home/translate-arrow.svg"
            transform={['rotate(0deg)', null, null, 'rotate(-90deg)']}
            w={['16px', null, null, '24px']}
          />
        </Flex>
        <TransInput
          blurPlaceholder={'예: 사분의 삼 → ⠼⠙⠌⠼⠉'}
          focusPlaceholder={'예: 사분의 삼 → ⠼⠙⠌⠼⠉'}
          readOnly
          value={braille}
        />
      </Flex>
    </VStack>
  )
}
