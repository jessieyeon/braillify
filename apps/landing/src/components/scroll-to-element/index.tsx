'use client'

import { Button } from '@devup-ui/react'
import { ComponentProps } from 'react'

export function ScrollToElement({
  elementId,
  onClick,
  ...props
}: ComponentProps<typeof Button<'button'>> & {
  elementId: string
}) {
  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    onClick?.(e)
    const element = document.getElementById(elementId)
    if (element) {
      element.scrollIntoView({ behavior: 'smooth' })
    }
  }
  return <Button onClick={handleClick} {...props} />
}
