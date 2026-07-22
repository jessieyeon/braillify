# 수학 점역 랜딩 데모 (MathLive) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 랜딩 페이지에 MathLive 기반 WYSIWYG 수식 입력기를 추가해, 사용자가 화면 수식 키보드로 수식을 편집하면 자동 생성된 LaTeX가 실시간으로 한국 수학 점자로 점역되는 데모 섹션을 만든다.

**Architecture:** 기존 `Trans`(텍스트 점역 데모) 섹션과 동일한 시각 언어를 따르는 새 `MathTrans` 섹션을 홈페이지에 추가한다. MathLive `<math-field>` 웹 컴포넌트가 LaTeX를 생성하면 `$...$`로 감싸 기존 braillify WASM API(`translateToUnicode`)에 그대로 넘긴다. **코어 엔진 변경은 없다** — `libs/braillify`의 `latex_math.rs`가 이미 `$\frac{3}{4}$` → `⠼⠙⠌⠼⠉`, `x^2`(중괄호 없는 위첨자), `\le`/`\leq`, `\times`, `\cdot`, `\pi` 등을 처리함을 CLI로 검증 완료.

**Tech Stack:** Next.js 16 (App Router), React 19, devup-ui, MathLive 0.110.x (웹 컴포넌트 + 가상 수식 키보드), braillify WASM (workspace 패키지)

## Global Constraints

- 브랜치: `demo-math-landing` (origin/main 기반, 이미 생성됨 — 기존 `demo-ios-app` 네이밍 패턴 `demo-<대상>` 준수)
- mathlive 버전: `^0.110.0`
- **외부 CDN 금지** — braillify는 "외부 연결 없이 실행"이 셀링 포인트이므로 MathLive 폰트를 `apps/landing/public/mathlive/fonts/`에 self-host 한다
- **코어 엔진(`libs/braillify`) 및 WASM 바인딩(`packages/node`) 수정 금지** — 랜딩 앱 안에서만 작업
- 분수는 LaTeX `\frac{}{}` 표기만 유효 (AGENTS.md NON-NEGOTIABLE) — MathLive는 `/` 입력 시 자동으로 `\frac{}{}`를 생성하므로 자연 충족
- `apps/landing`에는 테스트 인프라가 없다. 각 태스크의 검증 게이트는 `bun run build`(Next 빌드 + 타입체크)와 브라우저 확인이다
- UI 문구는 한국어, 기존 홈페이지 문체(“~해보세요!”) 유지
- 커밋 메시지는 기존 컨벤션(`feat:`, `fix:` 등) 준수

## 사전 완료 사항 (이 플랜 실행 전 이미 됨)

- [x] `demo-math-landing` 브랜치 생성 (origin/main 기반)
- [x] iOS 데모 WIP는 stash에 보존됨 (`git stash list` → "demo-ios-app WIP")

---

### Task 1: mathlive 의존성 추가 + 폰트 self-hosting

**Files:**
- Modify: `apps/landing/package.json` (dependencies에 mathlive 추가)
- Create: `apps/landing/public/mathlive/fonts/*` (node_modules에서 복사)

**Interfaces:**
- Produces: `import('mathlive')` 가능한 상태, `/mathlive/fonts/` 경로로 폰트 정적 서빙

- [ ] **Step 1: package.json에 mathlive 추가**

`apps/landing/package.json`의 `dependencies`에서 `"katex": "^0.17.0",` 바로 다음 줄에 추가:

```json
    "mathlive": "^0.110.0",
```

- [ ] **Step 2: 설치**

Run: `cd /Users/Yeon/braillify && bun install`
Expected: 에러 없이 완료, `bun.lock` 갱신됨

- [ ] **Step 3: MathLive 폰트를 public으로 복사**

bun 워크스페이스는 루트 `node_modules`에 호이스팅하므로 실제 위치를 확인 후 복사:

```bash
cd /Users/Yeon/braillify
FONTS_DIR=$(dirname "$(find node_modules apps/landing/node_modules -path '*mathlive/fonts/KaTeX_Main-Regular.woff2' 2>/dev/null | head -1)")
mkdir -p apps/landing/public/mathlive
cp -R "$FONTS_DIR" apps/landing/public/mathlive/fonts
ls apps/landing/public/mathlive/fonts | head
```

Expected: `KaTeX_AMS-Regular.woff2` 등 woff2 파일 목록 출력

- [ ] **Step 4: 빌드로 회귀 없음 확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully` — 아직 코드 변경이 없으므로 기존과 동일하게 통과

- [ ] **Step 5: Commit**

```bash
cd /Users/Yeon/braillify
git add apps/landing/package.json bun.lock apps/landing/public/mathlive
git commit -m "feat: 랜딩에 mathlive 의존성 추가 및 폰트 self-hosting"
```

---

### Task 2: MathTransInput — MathLive 필드 래퍼 컴포넌트

**Files:**
- Create: `apps/landing/src/app/MathTransInput.tsx`

**Interfaces:**
- Consumes: Task 1의 mathlive 패키지, `/mathlive/fonts` 정적 경로
- Produces: `MathTransInput({ onLatexChange: (latex: string) => void })` — 사용자가 수식을 편집할 때마다 placeholder 토큰이 제거된 LaTeX 문자열을 콜백으로 전달. 포커스 시 화면 수식 키보드 표시.

**설계 근거 (구현자가 알아야 할 것):**
- `<math-field>`는 웹 컴포넌트라 SSR 불가 → `'use client'` + `useEffect` 안에서 `import('mathlive')` 동적 로드, 로드 완료 전에는 자리 placeholder Box 렌더
- 가상 키보드가 데모의 핵심 → `math-virtual-keyboard-policy="manual"` + `focusin`/`focusout`에서 `window.mathVirtualKeyboard.show()/hide()` (기본 `auto` 정책은 터치 기기에서만 키보드를 띄우므로 데스크톱 데모에 부적합)
- LaTeX 추출은 `field.getValue('latex-without-placeholders')` — 기본 `.value`는 빈 인자를 `\placeholder{}`로 직렬화해 braillify가 점역할 수 없음
- `MathfieldElement.soundsDirectory = null`로 키보드 효과음 비활성 (사운드 파일 미제공)

- [ ] **Step 1: 컴포넌트 작성**

`apps/landing/src/app/MathTransInput.tsx` 생성:

```tsx
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
```

- [ ] **Step 2: 타입체크/빌드 통과 확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully` — 아직 어디서도 import하지 않지만 타입 선언(`React.JSX.IntrinsicElements` 증강)이 유효한지 이 단계에서 확인된다. `math-field` JSX 태그 관련 타입 에러가 나면 declaration 블록을 점검할 것 (React 19는 `React.JSX` 네임스페이스를 사용한다)

- [ ] **Step 3: Commit**

```bash
cd /Users/Yeon/braillify
git add apps/landing/src/app/MathTransInput.tsx
git commit -m "feat: MathLive 수식 입력 래퍼 컴포넌트 MathTransInput 추가"
```

---

### Task 3: MathTrans 섹션 — LaTeX → 점자 변환 + 홈페이지 연결

**Files:**
- Create: `apps/landing/src/app/MathTrans.tsx`
- Modify: `apps/landing/src/app/page.tsx:79-81` (`<Trans />` 래퍼 Flex 바로 아래에 동일 래퍼로 추가)

**Interfaces:**
- Consumes: Task 2의 `MathTransInput({ onLatexChange })`, 기존 `TransInput`(`apps/landing/src/app/TransInput.tsx` — `blurPlaceholder`/`focusPlaceholder`/`readOnly`/`value` props), braillify WASM의 `translateToUnicode(text: string): string`
- Produces: 홈페이지에 렌더되는 `MathTrans()` 섹션 (props 없음)

**설계 근거:**
- 변환 파이프라인: MathLive LaTeX → `` `$${latex}$` `` 로 감싸기 → `translateToUnicode` — 기존 `Trans.tsx`의 WASM 동적 로드 패턴을 따르되, 로드는 마운트 시 1회만 한다 (`Trans.tsx`의 `[input]` 의존성은 매 입력마다 재로드하는 기존 버그이므로 답습하지 않는다)
- 입력 중간 상태(`\frac{}{}` 등)는 점역 불가 → catch에서 안내 문구 반환, 데모상 허용
- 생성된 LaTeX를 코드 폰트로 노출해 "LaTeX 자동 생성"을 보여준다

- [ ] **Step 1: MathTrans 컴포넌트 작성**

`apps/landing/src/app/MathTrans.tsx` 생성:

```tsx
'use client'
import { Box, Flex, Image, Text, VStack } from '@devup-ui/react'
import { useEffect, useState } from 'react'

import { MathTransInput } from './MathTransInput'
import { TransInput } from './TransInput'

export function MathTrans() {
  const [latex, setLatex] = useState('')
  const [translateToUnicode, setTranslateToUnicode] = useState<
    (input: string) => string
  >(() => () => '')
  useEffect(() => {
    import('braillify').then((mod) => {
      setTranslateToUnicode(() => (input: string) => {
        try {
          return mod.translateToUnicode(input)
        } catch (e) {
          console.error(e)
          return '점역할 수 없는 수식이 포함되어 있습니다.'
        }
      })
    })
  }, [])

  const braille = latex.trim() ? translateToUnicode(`$${latex}$`) : ''

  return (
    <VStack flex="1" gap={['16px', null, null, '30px']}>
      <Flex
        alignItems="flex-start"
        gap={['10px', null, null, '20px']}
        justifyContent={['center', null, null, 'flex-start']}
      >
        <Box
          aria-label="Finger pointing image"
          bg="$text"
          flexShrink={0}
          h={['20px', null, null, '32px']}
          maskImage="url(/images/home/finger-point.svg)"
          maskPosition="center"
          maskRepeat="no-repeat"
          maskSize="contain"
          w={['17px', null, null, '28px']}
        />
        <Text color="$text" pos="relative" top="-2px" typography="mainTextSm">
          수식도 점자가 됩니다. 수식 키보드로 입력해 수학 점역을 체험해보세요!
        </Text>
      </Flex>
      <Flex
        alignItems="center"
        flexDirection={['column', null, null, 'row']}
        gap={['12px', null, null, '30px']}
        w="100%"
      >
        <VStack flex="1" gap="12px" w="100%">
          <MathTransInput onLatexChange={setLatex} />
          <Text
            color="$text"
            fontFamily="monospace"
            minH="1.5em"
            opacity={0.7}
            px={['16px', null, null, '40px']}
            wordBreak="break-all"
          >
            {latex ? `LaTeX: $${latex}$` : 'LaTeX가 자동으로 생성됩니다'}
          </Text>
        </VStack>
        <Flex>
          <Image
            alt=""
            display={['none', null, null, 'block']}
            mr="10px"
            src="/images/home/translate-arrow-circle.svg"
            w="16px"
          />
          <Image
            alt=""
            src="/images/home/translate-arrow.svg"
            transform={['rotate(0deg)', null, null, 'rotate(-90deg)']}
            w={['16px', null, null, '24px']}
          />
        </Flex>
        <TransInput
          blurPlaceholder={
            '수식을 입력하면 이곳에 수학 점자가 표시됩니다.\n예: 사분의 삼 → ⠼⠙⠌⠼⠉'
          }
          focusPlaceholder="수식을 입력하면 이곳에 수학 점자가 표시됩니다."
          readOnly
          value={braille}
        />
      </Flex>
    </VStack>
  )
}
```

- [ ] **Step 2: page.tsx에 섹션 추가**

`apps/landing/src/app/page.tsx` 수정. import에 추가:

```tsx
import { MathTrans } from './MathTrans'
```

`<Flex maxW="1520px" w="100%"><Trans /></Flex>` 바로 다음 줄에 추가:

```tsx
          <Flex maxW="1520px" w="100%">
            <MathTrans />
          </Flex>
```

- [ ] **Step 3: 빌드 통과 확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully`

- [ ] **Step 4: Commit**

```bash
cd /Users/Yeon/braillify
git add apps/landing/src/app/MathTrans.tsx apps/landing/src/app/page.tsx
git commit -m "feat: 홈페이지에 MathLive 기반 수학 점역 데모 섹션 추가"
```

---

### Task 4: 브라우저 검증 및 마무리

**Files:**
- 수정 없음 (검증 결과에 따른 스타일/버그 수정만)

**Interfaces:**
- Consumes: Task 3까지의 전체 결과물

- [ ] **Step 1: dev 서버 실행**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run dev`
Expected: `http://localhost:3000`에서 서비스 시작

- [ ] **Step 2: 브라우저에서 체크리스트 검증**

`http://localhost:3000` 접속 후 다음을 모두 확인 (브라우저 자동화 도구 사용 가능):

1. 기존 텍스트 점역 데모 아래에 수학 데모 섹션이 렌더된다
2. 수식 필드 클릭 시 **화면 수식 키보드**가 하단에 나타난다
3. 키보드로 `3`, `/`, `4` 입력 → 필드에 분수 WYSIWYG 렌더, LaTeX 표시줄에 `$\frac{3}{4}$`, 점자 출력에 `⠼⠙⠌⠼⠉`
4. `x`, `^`, `2` 입력 → 점자 출력에 `⠭⠘⠼⠃` 포함
5. 필드 밖 클릭 시 가상 키보드가 사라진다
6. 수식을 전부 지우면 점자 출력이 비고 placeholder가 다시 보인다
7. 콘솔에 폰트 404 에러가 없다 (`/mathlive/fonts/` 로드 확인)
8. 다크 테마 토글 시 수식 필드 글자가 읽을 수 있는 상태다 (수식 글자색이 배경에 묻히면 `MathTransInput`의 `math-field` style에 `color: 'var(--text)'` 추가)

Expected: 8개 항목 전부 통과. 실패 항목은 superpowers:systematic-debugging으로 원인을 찾아 수정 후 재검증

- [ ] **Step 3: 최종 빌드 재확인**

Run: `cd /Users/Yeon/braillify/apps/landing && bun run build`
Expected: `✓ Compiled successfully`

- [ ] **Step 4: 수정 사항이 있었다면 커밋**

```bash
cd /Users/Yeon/braillify
git add apps/landing/src
git commit -m "fix: 수학 점역 데모 브라우저 검증 후 스타일 보정"
```

(수정이 없었다면 이 커밋은 생략)
