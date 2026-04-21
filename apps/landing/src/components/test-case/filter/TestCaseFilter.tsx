'use client'

import { Center, Text } from '@devup-ui/react'

import {
  TestCaseFilter as TestCaseFilterType,
  useTestCase,
} from '../TestCaseProvider'

export function TestCaseFilter({
  value,
  children,
}: {
  value: TestCaseFilterType
  children: React.ReactNode
}) {
  const { options, onChangeOptions } = useTestCase()
  const isSelected = options.filters.includes(value)

  const handleClick = () => {
    onChangeOptions({
      filters: [value],
    })
  }

  return (
    <Center
      _active={
        !isSelected && {
          bg: '$menuActive',
        }
      }
      _hover={
        !isSelected && {
          bg: '$menuHover',
        }
      }
      bg={isSelected ? '$primary' : null}
      border="solid 1px $primary"
      borderRadius="1000px"
      cursor="pointer"
      flexDir="column"
      onClick={handleClick}
      px="20px"
      py="3px"
      transition="background-color 0.2s ease"
    >
      <Text
        color={isSelected ? '$base' : '$primary'}
        typography="body"
        whiteSpace="nowrap"
      >
        {children}
      </Text>
    </Center>
  )
}
