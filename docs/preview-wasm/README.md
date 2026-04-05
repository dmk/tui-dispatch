# preview-wasm

Docs live-preview binaries are built from real workspace examples:
- `examples/counter` (`counter` bin)
- `examples/minesweeper` (`minesweeper` bin)

Both examples use local `tinycrossterm` on `wasm32` to avoid `crossterm` build/runtime
issues in the browser target.

## Build

From repo root:

```bash
bash docs/scripts/build-preview-wasm.sh
```

Preview binaries:
- `preview-counter`
- `preview-minesweeper`

Outputs are copied to `docs/public/wasm/preview-*.wasm`.
