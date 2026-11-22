use notia_mermaid::RenderError;

fn render_ok(src: &str) -> String {
    let bytes = notia_mermaid::render(src).unwrap_or_else(|err| panic!("{err}"));
    String::from_utf8(bytes).unwrap_or_else(|err| panic!("{err}"))
}

fn render_err(src: &str) -> RenderError {
    notia_mermaid::render(src).expect_err("expected an error")
}

fn text_pos(svg: &str, label: &str) -> (i32, i32) {
    let needle = format!(">{label}</text>");
    let idx = svg
        .find(&needle)
        .unwrap_or_else(|| panic!("missing {label}"));
    let tag = svg[..idx]
        .rfind("<text ")
        .unwrap_or_else(|| panic!("missing text tag for {label}"));
    let tag = &svg[tag..idx];
    (attr(tag, "x"), attr(tag, "y"))
}

fn attr(tag: &str, name: &str) -> i32 {
    let key = format!("{name}=\"");
    let i = tag.find(&key).unwrap_or_else(|| panic!("missing {name}"));
    let rest = &tag[i + key.len()..];
    let end = rest.find('"').unwrap_or_else(|| panic!("unclosed {name}"));
    rest[..end].parse().unwrap_or_else(|err| panic!("{err}"))
}

#[test]
fn renderer_is_object_safe() {
    let renderer = notia_mermaid::LocalRenderer;
    let obj: &dyn notia_mermaid::MermaidRenderer = &renderer;
    let svg = obj.render("flowchart LR\nA[A] --> B[B]\n").unwrap();
    assert!(svg.starts_with(b"<?xml"));
    assert!(svg.ends_with(b"</svg>\n"));
}

#[test]
fn flowchart_shapes_and_direction() {
    let svg = render_ok("flowchart TD\nR[Rect]\nO(Round)\nS([Stadium])\nC((Circle))\nD{Diamond}\n");
    assert!(svg.contains(">Rect</text>"));
    assert!(svg.contains(">Round</text>"));
    assert!(svg.contains(">Stadium</text>"));
    assert!(svg.contains(">Circle</text>"));
    assert!(svg.contains(">Diamond</text>"));
    assert!(svg.contains("rx=\"2\""));
    assert!(svg.contains("rx=\"8\""));
    assert!(svg.contains("<ellipse "));
    assert!(svg.contains("<polygon "));
}

#[test]
fn lr_places_the_target_to_the_right() {
    let svg = render_ok("flowchart LR\nA[Alpha] --> B[Beta]\n");
    let (ax, _) = text_pos(&svg, "Alpha");
    let (bx, _) = text_pos(&svg, "Beta");
    assert!(bx > ax);
}

#[test]
fn td_and_tb_place_the_target_below() {
    for header in ["flowchart TD", "flowchart TB", "graph TD"] {
        let svg = render_ok(&format!("{header}\nA[Alpha] --> B[Beta]\n"));
        let (_, ay) = text_pos(&svg, "Alpha");
        let (_, by) = text_pos(&svg, "Beta");
        assert!(by > ay, "{header}");
    }
}

#[test]
fn graph_lr_places_the_target_to_the_right() {
    let svg = render_ok("graph LR\nA[Alpha] --> B[Beta]\n");
    let (ax, _) = text_pos(&svg, "Alpha");
    let (bx, _) = text_pos(&svg, "Beta");
    assert!(bx > ax);
}

#[test]
fn edge_styles_and_labels() {
    let dotted = render_ok("flowchart LR\nA[A] -.-> B[B]\n");
    assert!(dotted.contains("stroke-dasharray=\"6 4\""));
    let thick = render_ok("flowchart LR\nA[A] ==> B[B]\n");
    assert!(thick.contains("stroke-width=\"3\""));
    let plain = render_ok("flowchart LR\nA[A] --- B[B]\n");
    assert!(!plain.contains("<polygon "));
    let labeled = render_ok("flowchart TD\nA[A] -->|Yes| B[B]\n");
    assert!(labeled.contains(">Yes</text>"));
    let text_label = render_ok("flowchart LR\nA[A] -- Go --> B[B]\n");
    assert!(text_label.contains(">Go</text>"));
    let thick_label = render_ok("flowchart LR\nA[A] == Wide ==> B[B]\n");
    assert!(thick_label.contains(">Wide</text>"));
    assert!(thick_label.contains("stroke-width=\"3\""));
    let dot_label = render_ok("flowchart LR\nA[A] -. Dot .-> B[B]\n");
    assert!(dot_label.contains(">Dot</text>"));
}

#[test]
fn chain_is_top_to_bottom() {
    let svg = render_ok("flowchart TD\nA[A] --> B[B] --> C[C]\n");
    let (_, ay) = text_pos(&svg, "A");
    let (_, by) = text_pos(&svg, "B");
    let (_, cy) = text_pos(&svg, "C");
    assert!(by > ay && cy > by);
}

#[test]
fn comments_breaks_and_escapes() {
    let svg = render_ok("flowchart TD\n%% hidden\nA[\"line1<br/>line2\"]\n");
    assert!(svg.contains(">line1</text>"));
    assert!(svg.contains(">line2</text>"));
    assert!(!svg.contains("hidden"));
    let escaped = render_ok("flowchart TD\nA[\"<hi>&\"]\n");
    assert!(escaped.contains("&lt;hi&gt;&amp;"));
    assert!(!escaped.contains("<hi>"));
}

#[test]
fn class_id_is_a_node() {
    let svg = render_ok("flowchart TD\nclass[Foo]\n");
    assert!(svg.contains(">Foo</text>"));
}

#[test]
fn empty_flowchart_is_a_small_svg() {
    let svg = render_ok("flowchart TD\n%% nothing\n");
    assert!(svg.contains("<svg "));
    assert!(svg.contains("</svg>"));
}

#[test]
fn sequence_messages_notes_loop_and_alt() {
    let svg = render_ok(
        "sequenceDiagram\n\
         participant Alice\n\
         participant Bob as Robert\n\
         Alice->>Bob: Hello\n\
         Bob-->>Alice: Hi\n\
         Note right of Alice: Side\n\
         Note left of Bob: Left\n\
         Note over Alice,Bob: Both\n\
         loop Retry\n\
         Alice->>Bob: Ping\n\
         end\n\
         alt Ok\n\
         Bob->>Alice: Pong\n\
         else Fail\n\
         Bob->>Alice: Err\n\
         end\n",
    );
    for label in [
        "Alice", "Robert", "Hello", "Hi", "Side", "Left", "Both", "Retry", "Ping", "Ok", "Fail",
        "Pong", "Err",
    ] {
        assert!(svg.contains(&format!(">{label}</text>")), "missing {label}");
    }
    assert!(svg.contains("stroke-dasharray=\"6 4\""));
}

#[test]
fn implicit_participants_and_actor_keyword() {
    let svg = render_ok("sequenceDiagram\nactor Alice\nAlice->>Bob: Hi\n");
    assert!(svg.contains(">Alice</text>"));
    assert!(svg.contains(">Bob</text>"));
}

#[test]
fn solid_sequence_line_has_no_arrowhead() {
    let svg = render_ok("sequenceDiagram\nA->B: Line\n");
    assert!(svg.contains(">Line</text>"));
    assert!(!svg.contains("<polygon "));
}

#[test]
fn samples_match_committed_svg() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    for name in ["flowchart", "sequence"] {
        let src = std::fs::read_to_string(root.join(format!("{name}.mmd"))).unwrap();
        let svg = notia_mermaid::render(&src).unwrap();
        let expected = std::fs::read(root.join(format!("{name}.svg"))).unwrap_or_else(|_| {
            panic!("missing examples/{name}.svg; run cargo run --example render_samples")
        });
        assert_eq!(svg, expected, "{name}.svg drifted");
    }
}

#[test]
fn deterministic() {
    let src = "flowchart LR\nA[One] --> B[Two]\n";
    assert_eq!(
        notia_mermaid::render(src).unwrap(),
        notia_mermaid::render(src).unwrap()
    );
}

#[test]
fn unsupported_constructs() {
    let cases = [
        ("classDiagram\nA <|-- B\n", "classDiagram"),
        ("stateDiagram-v2\n[*] --> A\n", "stateDiagram-v2"),
        ("erDiagram\nA ||--o{ B : has\n", "erDiagram"),
        ("gantt\ntitle Plan\n", "gantt"),
        ("pie title Pets\n", "pie title Pets"),
        ("gitGraph\ncommit\n", "gitGraph"),
        ("journey\ntitle Day\n", "journey"),
        ("mindmap\nroot((idea))\n", "mindmap"),
        ("timeline\ntitle History\n", "timeline"),
        ("quadrantChart\n", "quadrantChart"),
        ("C4Context\n", "C4Context"),
        ("flowchart BT\nA --> B\n", "flowchart BT"),
        ("graph RL\nA --> B\n", "graph RL"),
        ("flowchart TD\nsubgraph core\nA-->B\nend\n", "subgraph core"),
        ("flowchart TD\nstyle A fill:#fff\n", "style A fill:#fff"),
        (
            "flowchart TD\nclassDef x fill:#fff\n",
            "classDef x fill:#fff",
        ),
        ("flowchart TD\nclass A foo\n", "class A foo"),
        (
            "flowchart TD\nclick A \"https://example.com\"\n",
            "click A \"https://example.com\"",
        ),
        (
            "flowchart TD\nlinkStyle 0 stroke:#f00\n",
            "linkStyle 0 stroke:#f00",
        ),
        ("flowchart TD\ndirection LR\n", "direction LR"),
        ("flowchart TD\nA[/flag/]\n", "A[/flag/]"),
        ("flowchart TD\nA[[sub]]\n", "A[[sub]]"),
        ("flowchart TD\nA>flag]\n", "A>flag]"),
        ("flowchart TD\nA[(db)]\n", "A[(db)]"),
        ("flowchart TD\nA{{hex}}\n", "A{{hex}}"),
        ("flowchart TD\nA[x]:::", "A[x]:::"),
        ("sequenceDiagram\npar both\nA->>B: x\nend\n", "par both"),
        ("sequenceDiagram\nopt maybe\nA->>B: x\nend\n", "opt maybe"),
        ("sequenceDiagram\nautonumber\n", "autonumber"),
        ("sequenceDiagram\nrect rgb(0,0,0)\nend\n", "rect rgb(0,0,0)"),
        ("sequenceDiagram\nactivate Alice\n", "activate Alice"),
        ("sequenceDiagram\nA->>+B: hi\n", "A->>+B: hi"),
        ("sequenceDiagram\nA-xB: no\n", "A-xB: no"),
        ("sequenceDiagram\nA-)B: async\n", "A-)B: async"),
        (
            "sequenceDiagram\ncreate participant Alice\n",
            "create participant Alice",
        ),
        ("sequenceDiagram\ncritical stop\nend\n", "critical stop"),
        ("sequenceDiagram\nbox Aqua Alice\n", "box Aqua Alice"),
    ];
    for (src, statement) in cases {
        let err = render_err(src);
        assert!(err.message.starts_with("unsupported:"), "{src:?} -> {err}");
        assert_eq!(err.statement.as_deref(), Some(statement), "{src}");
        assert!(err.line.is_some(), "{src}");
    }
}

#[test]
fn syntax_errors_are_not_unsupported() {
    let err = render_err("flowchart TD\nA[nope\n");
    assert!(!err.message.starts_with("unsupported"));
    assert_eq!(err.line, Some(2));
    let err = render_err("flowchart TD\nA -->\n");
    assert_eq!(err.line, Some(2));
    assert!(err.message.contains("target"));
    let err = render_err("   \n%% only\n");
    assert_eq!(err.message, "empty diagram");
    assert!(err.line.is_none());
    let err = render_err("sequenceDiagram\nelse x\n");
    assert!(err.message.contains("else"));
    let err = render_err("sequenceDiagram\nloop a\nA->>B: x\n");
    assert!(err.message.contains("unclosed"));
}

#[test]
fn limits_reject_without_building_a_huge_graph() {
    let err = render_err(&"x".repeat(70_000));
    assert!(err.message.contains("bytes"));
    assert!(err.line.is_none());

    let mut nodes = String::from("flowchart TD\n");
    for i in 0..250 {
        nodes.push_str(&format!("N{i}[n]\n"));
    }
    let err = render_err(&nodes);
    assert!(err.message.contains("nodes"), "{err}");

    let mut nested = String::from("sequenceDiagram\n");
    for _ in 0..20 {
        nested.push_str("loop x\n");
    }
    let err = render_err(&nested);
    assert!(err.message.contains("nested"), "{err}");
}

#[test]
fn malformed_inputs_finish_quickly() {
    let mut huge_line = String::from("flowchart TD\n");
    huge_line.push_str(&"[".repeat(20_000));
    let mut chained = String::from("flowchart TD\n");
    chained.push_str(&"A --> ".repeat(5_000));
    let samples = [
        String::new(),
        " \n%%\n".to_string(),
        "flowchart TD\n[\n".to_string(),
        "sequenceDiagram\nAlice->>\n".to_string(),
        "sequenceDiagram\nend\n".to_string(),
        huge_line,
        chained,
        "x".repeat(80_000),
    ];
    let start = std::time::Instant::now();
    for sample in &samples {
        let _ = notia_mermaid::render(sample);
    }
    assert!(
        start.elapsed().as_secs() < 5,
        "malformed input took {:?}",
        start.elapsed()
    );
}
