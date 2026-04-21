'use client'

import { keyframes, VStack } from '@devup-ui/react'
import { ComponentProps, useRef, useState } from 'react'
import { useEffect } from 'react'

const fadeIn = keyframes({
  from: {
    opacity: 0,
  },
  to: {
    opacity: 1,
  },
})

export default function Tooltip({
  ...props
}: ComponentProps<typeof VStack<'div'>>) {
  const [viewportWidth, setViewportWidth] = useState(0)
  const ref = useRef<HTMLDivElement>(null)
  if (typeof window !== 'undefined' && viewportWidth !== window.innerWidth)
    setViewportWidth(window.innerWidth)
  useEffect(() => {
    const handleResize = () => {
      setViewportWidth(window.innerWidth)
    }

    window.addEventListener('resize', handleResize)

    return () => {
      window.removeEventListener('resize', handleResize)
    }
  }, [])

  return (
    <VStack
      ref={(el) => {
        if (!el) return

        ref.current = el

        const mo = new ResizeObserver((entries) => {
          entries.forEach((entry) => {
            const target = entry.target as HTMLDivElement
            const { x, width } = target.getBoundingClientRect()
            if (x + width > viewportWidth) {
              target.style.right = '16px'
              target.style.left = 'auto'
            }
          })
        })

        mo.observe(el)

        return () => mo.disconnect()
      }}
      _groupHover={{
        display: 'flex',
        animationName: fadeIn,
        animationDuration: '0.2s',
        animationFillMode: 'forwards',
      }}
      bg="rgba(0, 0, 0, 0.75)"
      borderRadius="4px"
      display="none"
      justifyContent="center"
      maxW="calc(100vw - 32px)"
      onMouseEnter={(e) => {
        e.stopPropagation()
      }}
      onMouseLeave={(e) => {
        e.stopPropagation()
      }}
      opacity={0}
      pos="absolute"
      px="10px"
      py="8px"
      styleOrder={1}
      transform="translateY(10px)"
      zIndex="100"
      {...props}
    />
  )
}
