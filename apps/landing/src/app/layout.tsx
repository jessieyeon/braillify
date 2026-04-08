import { Box, globalCss, ThemeScript } from '@devup-ui/react'
import type { Metadata } from 'next'
import localFont from 'next/font/local'

import Footer from '@/components/Footer'
import Header from '@/components/Header'

const middleKoreanFont = localFont({
  src: './fonts/NanumBarunGothic-YetHangul.woff2',
  variable: '--font-middle-korean',
  display: 'swap',
})

export const metadata: Metadata = {
  title: 'Braillify',
  description: '크로스플랫폼 한국어 점역 라이브러리',
  keywords: [
    '한글 점역',
    '한글 점역 라이브러리',
    '한글 점역 라이브러리 추천',
    'Braillify',
    '브레일리파이',
    '점자 번역',
    '점자 번역기',
    '점자 통역',
    '점자 통역기',
    '점역기',
    '점자 번역 라이브러리',
    '점자 통역 라이브러리',
    '점자 번역 라이브러리 추천',
    '점자 통역 라이브러리 추천',
    'braille translator',
    '한글 점자',
    '영어 점자',
    '점역 서비스',
    '점자 ↔ 한글',
    '점자 ↔ 영어',
    'braille english',
    'braille korean',
    'braille translator',
  ],
  openGraph: {
    title: 'Braillify',
    description: '크로스플랫폼 한국어 점역 라이브러리',
    images: ['https://braillify.kr/og-image.png'],
    siteName: 'Braillify',
    type: 'website',
    url: 'https://braillify.kr',
  },
  metadataBase: new URL('https://braillify.kr'),
  alternates: {
    canonical: '/',
  },
}

globalCss({
  imports: ['https://spoqa.github.io/spoqa-han-sans/css/SpoqaHanSansNeo.css'],
  'html, body': {
    maxWidth: '100vw',
    overflowX: 'hidden',
  },
  body: {
    maxHeight: '100vh',
    background: '#373634',
    // WebkitFontSmoothing: 'antialiased',
    // MozOsxFontSmoothing: 'grayscale',
    fontFamily: 'Spoqa Han Sans Neo, Arial, Helvetica, sans-serif',
    wordBreak: 'keep-all',
  },
  '*': {
    boxSizing: 'border-box',
    padding: 0,
    margin: 0,
  },
  a: {
    color: '$link',
    textDecoration: 'none',
  },
  '::placeholder': {
    fontFamily: 'Spoqa Han Sans Neo, Arial, Helvetica, sans-serif',
  },
})

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="ko" suppressHydrationWarning>
      <head>
        <script>{`
(function(w,d,s,l,i){w[l]=w[l]||[];w[l].push({'gtm.start':
new Date().getTime(),event:'gtm.js'});var f=d.getElementsByTagName(s)[0],
j=d.createElement(s),dl=l!='dataLayer'?'&l='+l:'';j.async=true;j.src=
'https://www.googletagmanager.com/gtm.js?id='+i+dl;f.parentNode.insertBefore(j,f);
})(window,document,'script','dataLayer','GTM-KHQZ6Z4V')`}</script>
        <ThemeScript auto />
        <link href="/favicon.svg" rel="shortcut icon" />
      </head>
      <body className={middleKoreanFont.variable}>
        <noscript>
          <iframe
            height="0"
            src="https://www.googletagmanager.com/ns.html?id=GTM-KHQZ6Z4V"
            style={{ display: 'none', visibility: 'hidden' }}
            title="Google Tag Manager"
            width="0"
          />
        </noscript>
        <Box bg="$background" pos="relative">
          <Header />
          {children}
        </Box>
        <Footer />
      </body>
    </html>
  )
}
