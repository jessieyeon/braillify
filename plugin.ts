import { plugin } from 'bun'

plugin({
  name: 'wasm-pack-bun-loader',
  setup(build) {
    build.onLoad({ filter: /_bg\.wasm$/ }, async ({ path }) => {
      const jsPath = `${path.slice(0, -5)}.js`
      const glue = await import(jsPath)
      const bytes = await Bun.file(path).arrayBuffer()
      const instantiated = await WebAssembly.instantiate(bytes, {
        './index_bg.js': {
          __wbindgen_cast_0000000000000001:
            glue.__wbindgen_cast_0000000000000001,
          __wbindgen_init_externref_table: glue.__wbindgen_init_externref_table,
        },
      })
      const instance =
        instantiated instanceof WebAssembly.Instance
          ? instantiated
          : instantiated.instance

      return {
        exports: Object.fromEntries(Object.entries(instance.exports)),
        loader: 'object',
      }
    })
  },
})
