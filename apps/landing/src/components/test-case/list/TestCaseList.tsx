import { Box, Grid, Text } from '@devup-ui/react'
import Latex from 'react-syntax-highlighter/dist/cjs/languages/hljs/latex'

import { MIDDLE_KOREAN_FONT_FAMILY } from '@/constants/font'
import { TestStatus } from '@/types'

import TestCaseCircle from '../TestCaseCircle'
import { TestCaseDisplayBoundary } from '../TestCaseDisplayBoundary'

export function TestCaseList({ results }: { results: TestStatus[6] }) {
  return (
    <Grid gap="8px" gridTemplateColumns="repeat(auto-fill, minmax(16px, 1fr))">
      {results.map(
        ([
          text,
          note,
          expected,
          actual,
          isSuccess,
          world,
          worldIsSuccess,
          jeomsarang,
          jeomsarangIsSuccess,
        ]) => {
          const textParts = parseTextWithLaTeX(text)
          const testCaseKey = [
            text,
            note ?? '',
            expected,
            actual,
            world,
            jeomsarang,
          ].join('::')

          return (
            <TestCaseDisplayBoundary
              key={testCaseKey}
              option="failedOnly"
              value={Number(!isSuccess)}
            >
              <TestCaseCircle isSuccess={isSuccess}>
                <Box minW="50vw" w="100%" whiteSpace="pre-wrap">
                  <Text
                    color="#FFF"
                    fontFamily={MIDDLE_KOREAN_FONT_FAMILY}
                    typography="body"
                  >
                    {textParts.map((part) =>
                      part.type === 'latex' ? (
                        <Latex key={part.key}>${part.content}$</Latex>
                      ) : (
                        <span key={part.key}>{part.content}</span>
                      ),
                    )}
                    {note ? ` (${note})` : null}
                    <br />
                    정답 : <Text wordBreak="break-all">{expected}</Text>
                    <br />
                    결과 : <Text wordBreak="break-all">{actual}</Text>
                    <br />
                    {isSuccess ? '✅ 테스트 성공' : '❌ 테스트 실패'}
                    {world ? (
                      <>
                        <br />
                        점자세상 : <Text wordBreak="break-all">
                          {world}
                        </Text>{' '}
                        {worldIsSuccess ? '✅' : '❌'}
                      </>
                    ) : null}
                    {jeomsarang ? (
                      <>
                        <br />
                        점사랑 : <Text wordBreak="break-all">
                          {jeomsarang}
                        </Text>{' '}
                        {jeomsarangIsSuccess ? '✅' : '❌'}
                      </>
                    ) : null}
                  </Text>
                </Box>
              </TestCaseCircle>
            </TestCaseDisplayBoundary>
          )
        },
      )}
    </Grid>
  )
}

/**
 * This function parses text with LaTeX expressions and returns an array of parts.
 * It assumes that LaTeX is wrapped in double dollar delimiters ($$...$$).
 * Note that single dollar delimiters ($...$) are not rendered.
 * @param input - The input text to parse.
 * @returns An array of parts, where each part is either a text or a LaTeX expression.
 */
const parseTextWithLaTeX = (input: string) => {
  const parts: Array<{
    key: string
    type: 'text' | 'latex'
    content: string
  }> = []
  const latexRegex = /\$\$([^$]+(?:\$(?!\$)[^$]*)*)\$\$/g
  let lastIndex = 0
  let match: RegExpExecArray | null = latexRegex.exec(input)

  while (match !== null) {
    // if there is text before the LaTeX expression, add it as a text part:
    if (match.index > lastIndex) {
      const textContent = input.slice(lastIndex, match.index)
      if (textContent) {
        parts.push({
          key: `text-${lastIndex}-${match.index}`,
          type: 'text',
          content: textContent,
        })
      }
    }

    // add the LaTeX expression from double dollars:
    const latexContent = match[1]
    parts.push({
      key: `latex-${match.index}-${match[0].length}`,
      type: 'latex',
      content: latexContent,
    })
    lastIndex = match.index + match[0].length
    match = latexRegex.exec(input)
  }

  // add remaining text after the last LaTeX expression:
  if (lastIndex < input.length) {
    const remainingText = input.slice(lastIndex)
    if (remainingText) {
      parts.push({
        key: `text-${lastIndex}-${input.length}`,
        type: 'text',
        content: remainingText,
      })
    }
  }

  // if no LaTeX found, return the original text as a single text part:
  if (!parts.length) {
    parts.push({
      key: `text-0-${input.length}`,
      type: 'text',
      content: input,
    })
  }

  return parts
}
