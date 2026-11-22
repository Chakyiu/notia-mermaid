# notia-mermaid

Local Mermaid renderer used by Notia. It turns the text of a `mermaid` fence into SVG bytes. No network, no QuickJS, and no GPUI types.

v1 draws two diagram types. Everything else returns an error and does not panic.

## Host use

```rust
let svg = notia_mermaid::render(source)?;

let renderer = notia_mermaid::LocalRenderer;
let svg = notia_mermaid::MermaidRenderer::render(&renderer, source)?;
```

`MermaidRenderer` is object-safe. On `Err`, show the fence as a code block plus `error.message`. `error.line` is the 1-based line when the failure is on one line. `error.statement` is set for unsupported syntax (and for most syntax errors).

Depend on this crate from the Notia repo with a git dependency. Do not copy it into the app tree.

## Supported in v1

### `flowchart` and `graph`

- Directions: `TD`, `TB` (same as `TD`), `LR`. A missing direction means `TD`.
- Shapes: `A[rect]`, `A(round)`, `A([stadium])`, `A((circle))`, `A{diamond}`. Quoted labels (`A["a b"]`) are fine.
- Edges: `-->`, `---`, `-.->`, `==>`, with `|label|` or the text form `-- label -->`, `== label ==>`, `-. label .->`.
- Chains on one line: `A --> B --> C`. Several statements on one line can be split with `;`.
- `%%` comments, and `<br>` / `<br/>` inside a label (a new SVG line).

### `sequenceDiagram`

- `participant` and `actor`, including `as` aliases. A name used in a message is created if it was not declared.
- Messages: `->`, `-->`, `->>`, `-->>`.
- Notes: `Note left of`, `Note right of`, `Note over` (one participant or several).
- `loop` / `end` and `alt` / `else` / `end`, nested up to 8 deep.
- `%%` comments.

## Not supported

The error message is `unsupported: <statement>` and `statement` holds that line. This includes:

- Other diagram types (`classDiagram`, `stateDiagram`, `erDiagram`, `gantt`, `pie`, `gitGraph`, `journey`, `mindmap`, `timeline`, `quadrantChart`, `C4Context`, and the rest).
- Flowchart `subgraph`, `style`, `classDef`, `class` assignments, `click`, `linkStyle`, `direction`, and directions `BT` / `RL`.
- Shapes other than the five above (cylinder, hexagon, flag, parallelogram, subroutine).
- Sequence `par`, `opt`, `critical`, `break`, `rect`, `box`, `autonumber`, `activate` / `deactivate`, `create` / `destroy`, activation (`+` / `-` on an arrow), and cross or async arrows (`-x`, `-)`).

`%%{init}%%` directives are ignored with other `%%` comments. There is no theme and no network fallback.

## Limits

Oversized input returns an error instead of allocating without a cap:

- 64 KiB of source
- 2 000 lines
- 200 nodes
- 400 edges
- 500 statements
- 8 nested `loop` / `alt` blocks
- 500 characters in one label

## Layout

Flowchart edges are straight between node borders. A same-rank or backward edge is a right-angle detour. Sequence messages are horizontal, with lifelines under the participant boxes. This is a readable subset, not mermaid.live pixel parity.

## Samples

```bash
cargo run --example render_samples
```

That writes `examples/flowchart.svg` and `examples/sequence.svg`. `cargo test` checks those files still match the renderer.
