# 랜딩 데모 한글/수학 탭 통합 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 홈페이지의 한글 점역 데모와 수학 점역 데모(두 개의 세로 섹션)를 '한글'/'수학' 탭 토글 하나로 통합하고, 수학 안내문을 왼쪽(입력창 placeholder)으로 옮기며, 입력·출력 박스 크기를 동일하게 맞춘다.

**Architecture:** 새 클라이언트 컴포넌트 `DemoTabs`가 탭 상태(`'korean' | 'math'`)를 갖고 기존 `Trans` 또는 `MathTrans`를 조건부 렌더한다. `Trans`는 수정하지 않는다. `MathTransInput`은 TransInput과 동일한 박스 규격(minH 25dvh, 동일 padding/borderRadius)으로 바뀌고, 빈 상태 placeholder 오버레이와 LaTeX 미리보기 줄을 내장한다. `MathTrans`는 미리보기 렌더를 제거하고 데스크톱 500px 고정 행 높이로 좌우 박스를 대칭화한다.

**Tech Stack:** Next.js 16 (App Router), React 19, devup-ui, MathLive 0.110.x (이미 설치됨), braillify WASM

## Global Constraints

- 브랜치: `demo-math-landing` (이미 체크아웃됨), base `fa0c527`
- 코어 엔진(`libs/braillify`) 및 WASM 바인딩(`packages/node`) 수정 금지
- `apps/landing/src/app/Trans.tsx`, `TransInput.tsx`는 수정 금지 (한글 데모는 요청 범위 밖)
- 검증 게이트: `cd apps/landing && bun run build` → `✓ Compiled successfully` (테스트 인프라 없음)
- UI 문구 (verbatim, 사용자 지정):
  - 탭 라벨: `한글`, `수학`
  - 수학 입력창(왼쪽) placeholder: `수식을 입력하면 이곳에 수학 점자가 표시됩니다.`
  - 수학 출력창(오른쪽) placeholder(예시 유지): `예: 사분의 삼 → ⠼⠙⠌⠼⠉`
  - LaTeX 미리보기: 값 있을 때 `` LaTeX: $${latex}$ ``, 없을 때 `LaTeX가 자동으로 생성됩니다`
- 기본 선택 탭은 `한글`
- 커밋 메시지는 기존 컨벤션(`feat:`/`fix:` + 한국어) 준수

---

### Task 1: 수학 데모 좌측 안내문 이동 + 입출력 박스 동일 크기

**Files:**
- Modify: `apps/landing/src/app/MathTransInput.tsx` (컴포넌트 부분 전면 교체 — normalizeFracBraces/readArg와 declare global 블록은 그대로 둔다)
- Modify: `apps/landing/src/app/MathTrans.tsx`

**Interfaces:**
- Consumes: 기존 `TransInput`(수정 금지), `normalizeFracBraces` (같은 파일에 이미 존재)
- Produces: `MathTransInput({ onLatexChange: (latex: string) => void, placeholder: string })` — LaTeX 미리보기 줄을 자체 렌더하므로 `MathTrans`는 미리보기를 렌더하지 않는다. `MathTrans()`는 props 없음 (변경 없음, Task 2가 소비).

- [ ] **Step 1: MathTransInput.tsx의 컴포넌트를 교체**

파일 상단의 `'use client'`, import, `declare global`, `normalizeFracBraces`, `readArg`는 **그대로 두고**, `export function MathTransInput` 전체를 아래로 교체한다. import 줄은 다음으로 갱신한다:

```tsx
import { Box, Flex, Text } from '@devup-ui/react'
import type { MathfieldElement } from 'mathlive'
import { useEffect, useRef, useState } from 'react'
```

```tsx
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
```

설계 근거:
- 박스 규격(`bg`, `borderRadius`, `minH="25dvh"`, `p`)은 `TransInput.tsx`의 Box와 동일 — "입력·출력 박스 크기 동일" 요구
- placeholder는 math-field가 클릭을 받아야 하므로 `pointerEvents="none"` 오버레이. math-field 로드 후에는 필드 아래(top 48px)에 표시해 커서와 겹치지 않게 한다
- LaTeX 미리보기가 박스 안으로 들어와 좌우 박스 외곽 크기가 정확히 같아진다

- [ ] **Step 2: MathTrans.tsx 수정**

`apps/landing/src/app/MathTrans.tsx`에서 (a) 입력측 `VStack`+미리보기 `Text` 래퍼를 제거하고 `MathTransInput`을 직접 배치하며 placeholder를 전달, (b) 출력 placeholder를 예시만 남기고, (c) 행 레이아웃을 Trans와 동일한 데스크톱 500px 높이로 바꾼다. return문의 두 번째 `<Flex>`(행 레이아웃)를 아래로 교체:

```tsx
      <Flex
        alignItems="center"
        flexDirection={['column', null, null, 'row']}
        gap={['12px', null, null, '30px']}
        h={['auto', null, null, '500px']}
        w="100%"
      >
        <MathTransInput
          onLatexChange={setLatex}
          placeholder="수식을 입력하면 이곳에 수학 점자가 표시됩니다."
        />
        <Flex aria-hidden="true">
          <Image
            alt=""
            display={['none', null, null, 'block']}
            mr="10px"
            role="presentation"
            src="/images/home/translate-arrow-circle.svg"
            w="16px"
          />
          <Image
            alt=""
            role="presentation"
            src="/images/home/translate-arrow.svg"
            transform={['rotate(0deg)', null, null, 'rotate(-90deg)']}
            w={['16px', null, null, '24px']}
          />
        </Flex>
        <TransInput
          blurPlaceholder={'예: 사분의 삼 → ⠼⠙⠌⠼⠉'}
          focusPlaceholder={'예: 사분의 삼 → ⠼⠙⠌⠼⠉'}
          readOnly
          value={braille}
        />
      </Flex>
```

주의: `MathTransInput`은 이제 자체적으로 `flex-direction: column` Flex를 반환하지만 부모 행에서 `TransInput`과 동일하게 flex 아이템으로 늘어나야 한다. `TransInput`의 최외곽은 `<Flex flex="1" h="100%" ...>`이므로, `MathTransInput` 최외곽 Flex에도 `flex="1"`을 추가해야 좌우 너비가 같아진다 — Step 1 코드의 최외곽 `<Flex>`에 `flex="1"` prop을 포함할 것 (누락 시 이 Step에서 추가).

- [ ] **Step 3: 빌드 확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully`

- [ ] **Step 4: Commit**

```bash
cd /Users/Yeon/braillify
git add apps/landing/src/app/MathTransInput.tsx apps/landing/src/app/MathTrans.tsx
git commit -m "feat: 수학 데모 안내문을 입력창으로 이동하고 입출력 박스 크기 통일"
```

---

### Task 2: DemoTabs 탭 토글 + 홈페이지 통합

**Files:**
- Create: `apps/landing/src/app/DemoTabs.tsx`
- Modify: `apps/landing/src/app/page.tsx` (Trans/MathTrans 두 섹션 → DemoTabs 하나)

**Interfaces:**
- Consumes: `Trans()` (props 없음, `./Trans`), `MathTrans()` (props 없음, `./MathTrans`)
- Produces: `DemoTabs()` — props 없음, page.tsx가 렌더

- [ ] **Step 1: DemoTabs.tsx 생성**

```tsx
'use client'
import { Flex, Text, VStack } from '@devup-ui/react'
import { useState } from 'react'

import { MathTrans } from './MathTrans'
import { Trans } from './Trans'

const TABS = [
  { key: 'korean', label: '한글' },
  { key: 'math', label: '수학' },
] as const

type DemoMode = (typeof TABS)[number]['key']

export function DemoTabs() {
  const [mode, setMode] = useState<DemoMode>('korean')
  return (
    <VStack flex="1" gap={['16px', null, null, '30px']} w="100%">
      <Flex
        aria-label="점역 데모 종류"
        gap="10px"
        justifyContent={['center', null, null, 'flex-start']}
        role="tablist"
      >
        {TABS.map(({ key, label }) => (
          <Text
            key={key}
            aria-selected={mode === key}
            as="button"
            bg={mode === key ? '$text' : 'transparent'}
            border="1px solid $text"
            borderRadius="9999px"
            color={mode === key ? '$background' : '$text'}
            cursor="pointer"
            onClick={() => setMode(key)}
            px={['20px', null, null, '28px']}
            py={['8px', null, null, '10px']}
            role="tab"
            typography="button"
          >
            {label}
          </Text>
        ))}
      </Flex>
      {mode === 'korean' ? <Trans /> : <MathTrans />}
    </VStack>
  )
}
```

설계 근거:
- 탭 전환 시 `MathTransInput`이 언마운트되며 effect cleanup의 `window.mathVirtualKeyboard.hide()`가 열린 수식 키보드를 닫는다 (이미 구현됨)
- `Trans`/`MathTrans`는 각자의 안내 헤딩(손가락 아이콘 줄)을 유지하므로 탭 아래에서 문맥이 자연스럽다
- 색 토큰은 사이트 기존 `$text`/`$background` 반전 스타일

- [ ] **Step 2: page.tsx 수정**

`apps/landing/src/app/page.tsx`에서:

import 교체 — `import { Trans } from './Trans'`와 `import { MathTrans } from './MathTrans'`를 제거하고 추가:

```tsx
import { DemoTabs } from './DemoTabs'
```

본문에서 아래 두 섹션을:

```tsx
          <Flex maxW="1520px" w="100%">
            <Trans />
          </Flex>
          <Flex maxW="1520px" w="100%">
            <MathTrans />
          </Flex>
```

다음 하나로 교체:

```tsx
          <Flex maxW="1520px" w="100%">
            <DemoTabs />
          </Flex>
```

- [ ] **Step 3: 빌드 확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully`

- [ ] **Step 4: Commit**

```bash
cd /Users/Yeon/braillify
git add apps/landing/src/app/DemoTabs.tsx apps/landing/src/app/page.tsx
git commit -m "feat: 한글·수학 점역 데모를 탭 토글로 통합"
```

---

### Task 3: 브라우저 검증 및 마무리

**Files:**
- 수정 없음 (검증 결과에 따른 보정만)

- [ ] **Step 1: dev 서버 실행** (`bun run --cwd apps/landing dev`, 포트 3000)

- [ ] **Step 2: 체크리스트 검증**

1. 홈에 데모 섹션이 **하나만** 있고 상단에 `한글`/`수학` 탭이 보인다 (기본 `한글` 선택)
2. `한글` 탭: 기존 한글 데모가 그대로 동작 (입력 → 점자)
3. `수학` 탭 클릭: 수학 데모로 전환, 왼쪽 입력창에 안내문 "수식을 입력하면 이곳에 수학 점자가 표시됩니다."가 보인다
4. 오른쪽 출력창 placeholder가 "예: 사분의 삼 → ⠼⠙⠌⠼⠉"이다
5. 데스크톱 레이아웃에서 좌우 박스의 외곽 크기(너비·높이)가 동일하다 (getBoundingClientRect 비교, ±2px)
6. 수식 입력(VK 3÷4) → LaTeX 줄 `LaTeX: $\frac{3}{4}$` + 점자 `⠼⠙⠌⠼⠉`, placeholder 사라짐
7. 수학 탭에서 필드 포커스(키보드 열림) 상태로 `한글` 탭 전환 → 수식 키보드가 닫힌다
8. 콘솔 에러 없음

- [ ] **Step 3: 최종 빌드 재확인** (`bun run build` → `✓ Compiled successfully`)

- [ ] **Step 4: 보정이 있었다면 커밋** (`fix: 데모 탭 통합 브라우저 검증 후 보정`)
