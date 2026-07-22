'use client'

import { Box, Flex, Text } from '@devup-ui/react'
import type { MathfieldElement } from 'mathlive'
import { useEffect, useRef, useState } from 'react'

declare global {
  namespace React.JSX {
    interface IntrinsicElements {
      'math-field': React.DetailedHTMLProps<
        React.HTMLAttributes<MathfieldElement>,
        MathfieldElement
      > & { 'math-virtual-keyboard-policy'?: 'auto' | 'manual' }
    }
  }
}

/**
 * MathLive는 한 글자 인자를 중괄호 없이 직렬화한다 (예: \frac34, \frac{3}4).
 * braillify의 LaTeX 파서는 \frac{분자}{분모} 형태만 분수로 인식하므로
 * \frac의 두 인자를 항상 중괄호로 감싼 정규형으로 변환한다.
 */
export function normalizeFracBraces(latex: string): string {
  let out = ''
  let i = 0
  while (i < latex.length) {
    if (latex.startsWith('\\frac', i) && !/[a-zA-Z]/.test(latex[i + 5] ?? '')) {
      const [num, j] = readArg(latex, i + 5)
      const [den, k] = readArg(latex, j)
      out += `\\frac${num}${den}`
      i = k
      continue
    }
    out += latex[i]
    i += 1
  }
  return out
}

/** i 위치부터 LaTeX 인자 하나를 읽어 중괄호로 감싼 형태와 다음 인덱스를 돌려준다. */
function readArg(latex: string, i: number): [string, number] {
  while (latex[i] === ' ') i += 1
  if (latex[i] === '{') {
    let depth = 0
    let j = i
    do {
      if (latex[j] === '{') depth += 1
      else if (latex[j] === '}') depth -= 1
      j += 1
    } while (j < latex.length && depth > 0)
    return [`{${normalizeFracBraces(latex.slice(i + 1, j - 1))}}`, j]
  }
  if (latex[i] === '\\') {
    let j = i + 1
    while (j < latex.length && /[a-zA-Z]/.test(latex[j] ?? '')) j += 1
    return [`{${latex.slice(i, j)}}`, j]
  }
  return [`{${latex[i] ?? ''}}`, i + 1]
}

export function MathTransInput({
  onLatexChange,
  placeholder,
}: {
  onLatexChange: (latex: string) => void
  placeholder: string
}) {
  const [ready, setReady] = useState(false)
  const [latex, setLatex] = useState('')
  const fieldRef = useRef<MathfieldElement>(null)

  useEffect(() => {
    let cancelled = false
    import('mathlive').then(({ MathfieldElement }) => {
      if (cancelled) return
      MathfieldElement.fontsDirectory = '/mathlive/fonts'
      MathfieldElement.soundsDirectory = null
      setReady(true)
    })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    const field = fieldRef.current
    if (!ready || !field) return
    const show = () => window.mathVirtualKeyboard.show()
    const hide = () => window.mathVirtualKeyboard.hide()
    field.addEventListener('focusin', show)
    field.addEventListener('focusout', hide)
    return () => {
      field.removeEventListener('focusin', show)
      field.removeEventListener('focusout', hide)
      window.mathVirtualKeyboard.hide()
    }
  }, [ready])

  return (
    <Flex
      bg="$containerBackground"
      borderRadius={['16px', null, null, '30px']}
      flex="1"
      flexDirection="column"
      gap="12px"
      h="100%"
      minH="25dvh"
      p={['16px', null, null, '40px']}
      w="100%"
    >
      <Box flex="1" pos="relative">
        {ready && (
          <math-field
            ref={fieldRef}
            math-virtual-keyboard-policy="manual"
            onInput={(e) => {
              const value = normalizeFracBraces(
                (e.target as MathfieldElement).getValue(
                  'latex-without-placeholders',
                ),
              )
              setLatex(value)
              onLatexChange(value)
            }}
            style={{
              background: 'transparent',
              border: 'none',
              display: 'block',
              fontSize: '28px',
              width: '100%',
            }}
          />
        )}
        {!latex && (
          <Text
            color="$text"
            left="0"
            opacity={0.5}
            pointerEvents="none"
            pos="absolute"
            top={ready ? '48px' : '0'}
            typography="braille"
            whiteSpace="pre-line"
          >
            {placeholder}
          </Text>
        )}
      </Box>
      <Text
        color="$text"
        fontFamily="monospace"
        minH="1.5em"
        opacity={0.7}
        wordBreak="break-all"
      >
        {latex ? `LaTeX: $${latex}$` : 'LaTeX가 자동으로 생성됩니다'}
      </Text>
    </Flex>
  )
}
