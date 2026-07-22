'use client'

import { Box } from '@devup-ui/react'
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

export function MathTransInput({
  onLatexChange,
}: {
  onLatexChange: (latex: string) => void
}) {
  const [ready, setReady] = useState(false)
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
    }
  }, [ready])

  return (
    <Box
      bg="$containerBackground"
      borderRadius={['16px', null, null, '30px']}
      minH="120px"
      p={['16px', null, null, '40px']}
      w="100%"
    >
      {ready ? (
        <math-field
          ref={fieldRef}
          math-virtual-keyboard-policy="manual"
          onInput={(e) =>
            onLatexChange(
              (e.target as MathfieldElement).getValue(
                'latex-without-placeholders',
              ),
            )
          }
          style={{
            background: 'transparent',
            border: 'none',
            display: 'block',
            fontSize: '28px',
            width: '100%',
          }}
        />
      ) : (
        <Box minH="40px" />
      )}
    </Box>
  )
}
