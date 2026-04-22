'use client'

import { Toggle } from '@devup-ui/components'
import { css } from '@devup-ui/react'
import { ComponentProps } from 'react'

import { useTestCase } from './TestCaseProvider'

export function TestCaseTypeToggle(props: ComponentProps<typeof Toggle>) {
  const { options, onChangeOptions } = useTestCase()
  return (
    <Toggle
      className={css({
        selectors: { '& div': { bg: '$base' } },
      })}
      onChange={(value) =>
        onChangeOptions({ ...options, type: value ? 'table' : 'list' })
      }
      style={{
        backgroundColor: 'var(--primary)',
      }}
      value={options.type === 'table'}
      {...props}
    />
  )
}
