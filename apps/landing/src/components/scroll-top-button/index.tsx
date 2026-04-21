'use client'

import { Button } from '@devup-ui/react'
import { ComponentProps } from 'react'

export function ScrollTopButton(
  props: ComponentProps<typeof Button<'button'>>,
) {
  const handleClick = () => {
    document.body.scrollBy({
      top: -document.body.scrollHeight,
      behavior: 'smooth',
    })
  }
  return (
    <Button
      bg="transparent"
      border="none"
      onClick={handleClick}
      p="0"
      styleOrder={1}
      {...props}
    />
  )
}
