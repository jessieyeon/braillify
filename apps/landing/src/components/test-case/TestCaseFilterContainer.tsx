import { VStack } from '@devup-ui/react'

export function TestCaseFilterContainer({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <VStack
      bg="$background"
      gap="12px"
      pos="sticky"
      pt="10px"
      px={[null, null, null, '60px']}
      top={['60px', null, null, '100px']}
      zIndex="100"
    >
      {children}
    </VStack>
  )
}
