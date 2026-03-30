export type Merge<T, U> = Omit<T, Extract<keyof T, keyof U>> & U

export type TestStatus = [
  success: number,
  fail: number,
  Array<
    [
      text: string,
      note: string,
      expected: string,
      actual: string,
      isSuccess: boolean,
    ]
  >,
]

export type TestStatusMap = Record<string, TestStatus>
