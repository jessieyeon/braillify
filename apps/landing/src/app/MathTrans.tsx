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
          aria-label="Finger pointing image"
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
        w="100%"
      >
        <VStack flex="1" gap="12px" w="100%">
          <MathTransInput onLatexChange={setLatex} />
          <Text
            color="$text"
            fontFamily="monospace"
            minH="1.5em"
            opacity={0.7}
            px={['16px', null, null, '40px']}
            wordBreak="break-all"
          >
            {latex ? `LaTeX: $${latex}$` : 'LaTeX가 자동으로 생성됩니다'}
          </Text>
        </VStack>
        <Flex>
          <Image
            alt=""
            display={['none', null, null, 'block']}
            mr="10px"
            src="/images/home/translate-arrow-circle.svg"
            w="16px"
          />
          <Image
            alt=""
            src="/images/home/translate-arrow.svg"
            transform={['rotate(0deg)', null, null, 'rotate(-90deg)']}
            w={['16px', null, null, '24px']}
          />
        </Flex>
        <TransInput
          blurPlaceholder={
            '수식을 입력하면 이곳에 수학 점자가 표시됩니다.\n예: 사분의 삼 → ⠼⠙⠌⠼⠉'
          }
          focusPlaceholder="수식을 입력하면 이곳에 수학 점자가 표시됩니다."
          readOnly
          value={braille}
        />
      </Flex>
    </VStack>
  )
}
