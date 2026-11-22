use std::collections::HashMap;

use crate::error::RenderError;
use crate::limits::{self, Budget, MAX_EDGES, MAX_NODES};
use crate::scan::{self, Cursor};
use crate::svg::{self, FILL, PAD, STROKE, Svg};

pub(crate) fn render(source: &str) -> Result<Vec<u8>, RenderError> {
    let parsed = parse(source)?;
    Ok(layout(parsed))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FlowDir {
    Td,
    Lr,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Rect,
    Round,
    Stadium,
    Circle,
    Diamond,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
    Arrow,
    Line,
    Dotted,
    Thick,
}

struct Node {
    lines: Vec<String>,
    shape: Shape,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

struct Edge {
    from: usize,
    to: usize,
    kind: EdgeKind,
    label: Option<String>,
}

struct Graph {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    edges: Vec<Edge>,
}

struct Parsed {
    dir: FlowDir,
    graph: Graph,
}

fn parse(source: &str) -> Result<Parsed, RenderError> {
    let mut dir = None;
    let mut graph = Graph {
        nodes: Vec::new(),
        index: HashMap::new(),
        edges: Vec::new(),
    };
    let mut budget = Budget::new();
    for (idx, raw) in source.lines().enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let line = scan::strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if dir.is_none() {
            dir = Some(parse_header(line, line_no)?);
            continue;
        }
        for piece in split_statements(line) {
            let stmt = scan::strip_comment(piece)
                .trim()
                .trim_end_matches(';')
                .trim();
            if stmt.is_empty() {
                continue;
            }
            budget.bump(line_no, stmt)?;
            if is_reserved(stmt) {
                return Err(RenderError::unsupported(line_no, stmt));
            }
            parse_chain(&mut graph, stmt, line_no)?;
        }
    }
    Ok(Parsed {
        dir: dir.ok_or_else(RenderError::empty)?,
        graph,
    })
}

fn parse_header(line: &str, line_no: u32) -> Result<FlowDir, RenderError> {
    let line = line.trim_end_matches(';').trim();
    let mut parts = line.split_whitespace();
    let keyword = parts.next().unwrap_or("");
    if keyword != "flowchart" && keyword != "graph" {
        return Err(RenderError::invalid(
            line_no,
            "expected flowchart or graph",
            line,
        ));
    }
    match parts.next() {
        None | Some("TD" | "TB") => {
            if parts.next().is_some() {
                return Err(RenderError::invalid(
                    line_no,
                    "unexpected text after the direction",
                    line,
                ));
            }
            Ok(FlowDir::Td)
        }
        Some("LR") => {
            if parts.next().is_some() {
                return Err(RenderError::invalid(
                    line_no,
                    "unexpected text after the direction",
                    line,
                ));
            }
            Ok(FlowDir::Lr)
        }
        Some(_) => Err(RenderError::unsupported(line_no, line)),
    }
}

fn is_reserved(stmt: &str) -> bool {
    const ALWAYS: &[&str] = &[
        "subgraph",
        "style",
        "classDef",
        "click",
        "linkStyle",
        "direction",
        "accTitle",
        "accDescr",
        "interpolate",
    ];
    if ALWAYS.iter().any(|word| scan::starts_with_word(stmt, word)) {
        return true;
    }
    if scan::starts_with_word(stmt, "end") {
        let rest = stmt["end".len()..].trim_start();
        return !continues_as_node(rest);
    }
    if scan::starts_with_word(stmt, "class") {
        let rest = stmt["class".len()..].trim_start();
        return !rest.is_empty() && !continues_as_node(rest);
    }
    false
}

fn continues_as_node(rest: &str) -> bool {
    rest.starts_with('[')
        || rest.starts_with('(')
        || rest.starts_with('{')
        || rest.starts_with('-')
        || rest.starts_with('=')
        || rest.starts_with('.')
}

fn split_statements(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut square = 0i32;
    let mut paren = 0i32;
    let mut brace = 0i32;
    let mut quote: Option<char> = None;
    for (i, c) in line.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '[' => square += 1,
            ']' => square = square.saturating_sub(1),
            '(' => paren += 1,
            ')' => paren = paren.saturating_sub(1),
            '{' => brace += 1,
            '}' => brace = brace.saturating_sub(1),
            ';' if square == 0 && paren == 0 && brace == 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

struct PendingEdge {
    kind: EdgeKind,
    label: Option<String>,
}

fn parse_chain(graph: &mut Graph, stmt: &str, line_no: u32) -> Result<(), RenderError> {
    let mut cur = Cursor::new(stmt);
    let mut prev: Option<usize> = None;
    let mut pending: Option<PendingEdge> = None;
    let mut expect_node = true;
    loop {
        cur.skip_ws();
        if cur.eof() {
            break;
        }
        if expect_node {
            let idx = parse_node_ref(&mut cur, graph, line_no, stmt)?;
            if let Some(edge) = pending.take() {
                let from = prev.ok_or_else(|| {
                    RenderError::invalid(line_no, "edge is missing a source", stmt)
                })?;
                push_edge(graph, from, idx, edge, line_no, stmt)?;
            }
            prev = Some(idx);
            expect_node = false;
        } else if let Some(edge) = parse_edge(&mut cur, line_no, stmt)? {
            pending = Some(edge);
            expect_node = true;
        } else if cur.rest().starts_with(":::") {
            return Err(RenderError::unsupported(line_no, stmt));
        } else {
            return Err(RenderError::invalid(
                line_no,
                "unexpected text in the statement",
                stmt,
            ));
        }
    }
    if pending.is_some() {
        return Err(RenderError::invalid(
            line_no,
            "edge is missing a target",
            stmt,
        ));
    }
    if prev.is_none() {
        return Err(RenderError::invalid(line_no, "expected a node", stmt));
    }
    Ok(())
}

fn parse_node_ref(
    cur: &mut Cursor<'_>,
    graph: &mut Graph,
    line_no: u32,
    stmt: &str,
) -> Result<usize, RenderError> {
    let Some(id) = parse_id(cur) else {
        return Err(RenderError::invalid(line_no, "expected a node", stmt));
    };
    let shape = parse_shape(cur, line_no, stmt)?;
    if cur.rest().starts_with(":::") {
        return Err(RenderError::unsupported(line_no, stmt));
    }
    touch_node(graph, id, shape, line_no, stmt)
}

fn parse_id(cur: &mut Cursor<'_>) -> Option<String> {
    let rest = cur.rest();
    let mut len = 0;
    for (i, c) in rest.char_indices() {
        let ok = if i == 0 {
            c.is_ascii_alphabetic() || c == '_'
        } else {
            c.is_ascii_alphanumeric() || c == '_'
        };
        if !ok {
            break;
        }
        len = i + c.len_utf8();
    }
    if len == 0 {
        return None;
    }
    let id = rest[..len].to_string();
    cur.bump(len);
    Some(id)
}

fn parse_shape(
    cur: &mut Cursor<'_>,
    line_no: u32,
    stmt: &str,
) -> Result<Option<(Shape, String)>, RenderError> {
    let rest = cur.rest();
    if rest.starts_with("(((")
        || rest.starts_with("[[")
        || rest.starts_with("[(")
        || rest.starts_with("[/")
        || rest.starts_with("[\\")
        || rest.starts_with("{{")
        || rest.starts_with('>')
    {
        return Err(RenderError::unsupported(line_no, stmt));
    }
    let (shape, open, close) = if rest.starts_with("([") {
        (Shape::Stadium, 2, "])")
    } else if rest.starts_with("((") {
        (Shape::Circle, 2, "))")
    } else if rest.starts_with('(') {
        (Shape::Round, 1, ")")
    } else if rest.starts_with('[') {
        (Shape::Rect, 1, "]")
    } else if rest.starts_with('{') {
        (Shape::Diamond, 1, "}")
    } else {
        return Ok(None);
    };
    let (label, consumed) = take_label(&rest[open..], close, line_no, stmt)?;
    limits::check_label(&label, line_no, stmt)?;
    cur.bump(open + consumed);
    Ok(Some((shape, label)))
}

fn take_label(
    body: &str,
    close: &str,
    line_no: u32,
    stmt: &str,
) -> Result<(String, usize), RenderError> {
    let unclosed = || RenderError::invalid(line_no, "unclosed node label", stmt);
    let mut chars = body.char_indices();
    if let Some((_, q @ ('"' | '\''))) = chars.next() {
        for (i, c) in chars {
            if c == q {
                let after = &body[i + c.len_utf8()..];
                if after.starts_with(close) {
                    return Ok((body[1..i].to_string(), i + c.len_utf8() + close.len()));
                }
                return Err(unclosed());
            }
        }
        return Err(unclosed());
    }
    match body.find(close) {
        Some(pos) => Ok((body[..pos].trim().to_string(), pos + close.len())),
        None => Err(unclosed()),
    }
}

fn touch_node(
    graph: &mut Graph,
    id: String,
    shape: Option<(Shape, String)>,
    line_no: u32,
    stmt: &str,
) -> Result<usize, RenderError> {
    if let Some(&idx) = graph.index.get(&id) {
        if let Some((shape, label)) = shape {
            let lines = checked_lines(&label, line_no, stmt)?;
            let (w, h) = measure(shape, &lines);
            let node = &mut graph.nodes[idx];
            node.shape = shape;
            node.lines = lines;
            node.w = w;
            node.h = h;
        }
        return Ok(idx);
    }
    if graph.nodes.len() >= MAX_NODES {
        return Err(RenderError::limit_at(
            line_no,
            format!("diagram exceeds {MAX_NODES} nodes"),
            stmt,
        ));
    }
    let (shape, label) = shape.unwrap_or_else(|| (Shape::Rect, id.clone()));
    let lines = checked_lines(&label, line_no, stmt)?;
    let (w, h) = measure(shape, &lines);
    let idx = graph.nodes.len();
    graph.nodes.push(Node {
        lines,
        shape,
        x: 0,
        y: 0,
        w,
        h,
    });
    graph.index.insert(id, idx);
    Ok(idx)
}

fn checked_lines(label: &str, line_no: u32, stmt: &str) -> Result<Vec<String>, RenderError> {
    let lines = svg::label_lines(label);
    if lines.len() > 8 {
        return Err(RenderError::invalid(
            line_no,
            "label has too many lines",
            stmt,
        ));
    }
    Ok(lines)
}

fn measure(shape: Shape, lines: &[String]) -> (i32, i32) {
    let text_w = lines
        .iter()
        .map(|line| svg::text_width(line))
        .max()
        .unwrap_or(0);
    let text_h = (lines.len() as i32) * 18;
    match shape {
        Shape::Rect | Shape::Round => ((text_w + 28).max(72), (text_h + 16).max(36)),
        Shape::Stadium => ((text_w + 36).max(80), (text_h + 16).max(36)),
        Shape::Circle => {
            let d = (text_w.max(text_h) + 28).max(48);
            (d, d)
        }
        Shape::Diamond => ((text_w + 56).max(96), (text_h + 40).max(64)),
    }
}

fn parse_edge(
    cur: &mut Cursor<'_>,
    line_no: u32,
    stmt: &str,
) -> Result<Option<PendingEdge>, RenderError> {
    let rest = cur.rest();
    let Some((kind, label, consumed)) = match_edge(rest, line_no, stmt)? else {
        if rest.starts_with('-') || rest.starts_with('=') || rest.starts_with('.') {
            return Err(RenderError::unsupported(line_no, stmt));
        }
        return Ok(None);
    };
    if let Some(label) = &label {
        limits::check_label(label, line_no, stmt)?;
    }
    cur.bump(consumed);
    Ok(Some(PendingEdge { kind, label }))
}

fn match_edge(
    rest: &str,
    line_no: u32,
    stmt: &str,
) -> Result<Option<(EdgeKind, Option<String>, usize)>, RenderError> {
    if let Some(after) = rest.strip_prefix("-.->") {
        let (label, n) = pipe_label(after, line_no, stmt)?;
        return Ok(Some((EdgeKind::Dotted, label, "-.->".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("-->") {
        let (label, n) = pipe_label(after, line_no, stmt)?;
        return Ok(Some((EdgeKind::Arrow, label, "-->".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("==>") {
        let (label, n) = pipe_label(after, line_no, stmt)?;
        return Ok(Some((EdgeKind::Thick, label, "==>".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("---") {
        let (label, n) = pipe_label(after, line_no, stmt)?;
        return Ok(Some((EdgeKind::Line, label, "---".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("-.")
        && !after.starts_with('>')
        && let Some((label, n)) = text_until(after, ".->")
    {
        return Ok(Some((EdgeKind::Dotted, blank_none(label), "-.".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("==")
        && !after.starts_with('>')
        && let Some((label, n)) = text_until(after, "==>")
    {
        return Ok(Some((EdgeKind::Thick, blank_none(label), "==".len() + n)));
    }
    if let Some(after) = rest.strip_prefix("--")
        && !after.starts_with('>')
        && !after.starts_with('-')
    {
        if let Some((label, n)) = text_until(after, "-->") {
            return Ok(Some((EdgeKind::Arrow, blank_none(label), "--".len() + n)));
        }
        if let Some((label, n)) = text_until(after, "---") {
            return Ok(Some((EdgeKind::Line, blank_none(label), "--".len() + n)));
        }
    }
    Ok(None)
}

fn pipe_label(
    rest: &str,
    line_no: u32,
    stmt: &str,
) -> Result<(Option<String>, usize), RenderError> {
    let trimmed = rest.trim_start();
    let ws = rest.len() - trimmed.len();
    let Some(after) = trimmed.strip_prefix('|') else {
        return Ok((None, 0));
    };
    let Some(end) = after.find('|') else {
        return Err(RenderError::invalid(line_no, "unclosed edge label", stmt));
    };
    Ok((blank_none(after[..end].trim()), ws + end + 2))
}

fn text_until(body: &str, end: &str) -> Option<(String, usize)> {
    let pos = body.find(end)?;
    Some((body[..pos].trim().to_string(), pos + end.len()))
}

fn blank_none(label: impl AsRef<str>) -> Option<String> {
    let label = label.as_ref();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

fn push_edge(
    graph: &mut Graph,
    from: usize,
    to: usize,
    edge: PendingEdge,
    line_no: u32,
    stmt: &str,
) -> Result<(), RenderError> {
    if graph.edges.len() >= MAX_EDGES {
        return Err(RenderError::limit_at(
            line_no,
            format!("diagram exceeds {MAX_EDGES} edges"),
            stmt,
        ));
    }
    graph.edges.push(Edge {
        from,
        to,
        kind: edge.kind,
        label: edge.label,
    });
    Ok(())
}

fn layout(parsed: Parsed) -> Vec<u8> {
    let Parsed { dir, graph } = parsed;
    let Graph {
        mut nodes, edges, ..
    } = graph;
    if nodes.is_empty() {
        return Svg::new().finish(80, 48);
    }
    let edges_idx: Vec<(usize, usize)> = edges.iter().map(|edge| (edge.from, edge.to)).collect();
    let ranks = assign_ranks(nodes.len(), &edges_idx);
    let groups = rank_groups(&ranks);
    place(&mut nodes, &groups, dir);

    // shortcut: straight edges between borders; switch to orthogonal routing if diagrams overlap.
    let mut min_x = PAD;
    let mut max_x = 0;
    let mut max_y = 0;
    for node in &nodes {
        min_x = min_x.min(node.x);
        max_x = max_x.max(node.x + node.w);
        max_y = max_y.max(node.y + node.h);
    }
    for edge in &edges {
        let from = &nodes[edge.from];
        let to = &nodes[edge.to];
        if edge.from == edge.to {
            let label_w = edge.label.as_deref().map(svg::text_width).unwrap_or(0);
            let cx = from.x + from.w / 2;
            min_x = min_x.min(cx - label_w / 2);
            max_x = max_x.max(cx + label_w / 2);
            max_y = max_y.max(from.y + from.h + 48);
            continue;
        }
        if ranks[edge.from] == ranks[edge.to] {
            match dir {
                FlowDir::Td => {
                    max_y = max_y.max(from.y.max(to.y) + from.h.max(to.h) + 36);
                }
                FlowDir::Lr => {
                    max_x = max_x.max(from.x.max(to.x) + from.w.max(to.w) + 36);
                }
            }
        } else if ranks[edge.to] < ranks[edge.from] {
            match dir {
                FlowDir::Td => {
                    max_x = max_x.max(from.x.max(to.x) + from.w.max(to.w) + 36);
                }
                FlowDir::Lr => {
                    max_y = max_y.max(from.y.max(to.y) + from.h.max(to.h) + 36);
                }
            }
        }
    }
    let shift = if min_x < PAD { PAD - min_x } else { 0 };
    if shift > 0 {
        for node in &mut nodes {
            node.x += shift;
        }
        max_x += shift;
    }
    let width = max_x + PAD;
    let height = max_y + PAD;

    let mut svg = Svg::new();
    for edge in &edges {
        draw_edge(&mut svg, &nodes, &ranks, dir, edge);
    }
    for node in &nodes {
        draw_node(&mut svg, node);
    }
    svg.finish(width, height)
}

fn assign_ranks(n: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut rank = vec![0usize; n];
    for _ in 0..n {
        let mut changed = false;
        for &(from, to) in edges {
            if from == to {
                continue;
            }
            let next = rank[from].saturating_add(1);
            if next > rank[to] && next <= n {
                rank[to] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut order = rank.clone();
    order.sort_unstable();
    order.dedup();
    rank.into_iter()
        .map(|value| order.iter().position(|item| *item == value).unwrap_or(0))
        .collect()
}

fn rank_groups(ranks: &[usize]) -> Vec<Vec<usize>> {
    let max = ranks.iter().copied().max().unwrap_or(0);
    let mut groups = vec![Vec::new(); max + 1];
    for (i, rank) in ranks.iter().enumerate() {
        groups[*rank].push(i);
    }
    groups
}

const NODE_GAP: i32 = 32;
const RANK_GAP: i32 = 72;

fn place(nodes: &mut [Node], groups: &[Vec<usize>], dir: FlowDir) {
    match dir {
        FlowDir::Td => {
            let widths: Vec<i32> = groups
                .iter()
                .map(|group| span(nodes, group, true))
                .collect();
            let max_w = widths.iter().copied().max().unwrap_or(0);
            let mut y = PAD;
            for (group, width) in groups.iter().zip(widths) {
                let row_h = group.iter().map(|&i| nodes[i].h).max().unwrap_or(0);
                let mut x = PAD + (max_w - width) / 2;
                for &i in group {
                    nodes[i].x = x;
                    nodes[i].y = y + (row_h - nodes[i].h) / 2;
                    x += nodes[i].w + NODE_GAP;
                }
                y += row_h + RANK_GAP;
            }
        }
        FlowDir::Lr => {
            let heights: Vec<i32> = groups
                .iter()
                .map(|group| span(nodes, group, false))
                .collect();
            let max_h = heights.iter().copied().max().unwrap_or(0);
            let mut x = PAD;
            for (group, height) in groups.iter().zip(heights) {
                let col_w = group.iter().map(|&i| nodes[i].w).max().unwrap_or(0);
                let mut y = PAD + (max_h - height) / 2;
                for &i in group {
                    nodes[i].y = y;
                    nodes[i].x = x + (col_w - nodes[i].w) / 2;
                    y += nodes[i].h + NODE_GAP;
                }
                x += col_w + RANK_GAP;
            }
        }
    }
}

fn span(nodes: &[Node], group: &[usize], horizontal: bool) -> i32 {
    if group.is_empty() {
        return 0;
    }
    let sum: i32 = group
        .iter()
        .map(|&i| if horizontal { nodes[i].w } else { nodes[i].h })
        .sum();
    let gaps = NODE_GAP * (group.len() as i32 - 1);
    sum + gaps
}

fn draw_node(svg: &mut Svg, node: &Node) {
    let (x, y, w, h) = (node.x, node.y, node.w, node.h);
    match node.shape {
        Shape::Rect => svg.rect(x, y, w, h, 2, FILL, STROKE, None),
        Shape::Round => svg.rect(x, y, w, h, 8, FILL, STROKE, None),
        Shape::Stadium => svg.rect(x, y, w, h, h / 2, FILL, STROKE, None),
        Shape::Circle => svg.ellipse(x + w / 2, y + h / 2, w / 2, h / 2),
        Shape::Diamond => {
            let cx = x + w / 2;
            let cy = y + h / 2;
            svg.polygon(
                &[(cx, y), (x + w, cy), (cx, y + h), (x, cy)],
                FILL,
                Some(STROKE),
            );
        }
    }
    svg.text_lines(x + w / 2, y + h / 2, &node.lines);
}

fn draw_edge(svg: &mut Svg, nodes: &[Node], ranks: &[usize], dir: FlowDir, edge: &Edge) {
    let from = &nodes[edge.from];
    let to = &nodes[edge.to];
    if edge.from == edge.to {
        draw_self_loop(svg, from, edge);
        return;
    }
    if ranks[edge.from] == ranks[edge.to] {
        elbow_same(svg, from, to, dir, edge);
        return;
    }
    if ranks[edge.to] < ranks[edge.from] {
        elbow_back(svg, from, to, dir, edge);
        return;
    }
    let (x1, y1) = anchor(from, dir, true);
    let (x2, y2) = anchor(to, dir, false);
    paint_line(svg, x1, y1, x2, y2, edge);
}

fn anchor(node: &Node, dir: FlowDir, outgoing: bool) -> (i32, i32) {
    match (dir, outgoing) {
        (FlowDir::Td, true) => (node.x + node.w / 2, node.y + node.h),
        (FlowDir::Td, false) => (node.x + node.w / 2, node.y),
        (FlowDir::Lr, true) => (node.x + node.w, node.y + node.h / 2),
        (FlowDir::Lr, false) => (node.x, node.y + node.h / 2),
    }
}

fn elbow_same(svg: &mut Svg, from: &Node, to: &Node, dir: FlowDir, edge: &Edge) {
    match dir {
        FlowDir::Td => {
            let y = from.y.max(to.y) + from.h.max(to.h) + 22;
            let x1 = from.x + from.w / 2;
            let x2 = to.x + to.w / 2;
            let y1 = from.y + from.h;
            let y2 = to.y + to.h;
            paint_orth(svg, &[(x1, y1), (x1, y), (x2, y), (x2, y2)], edge);
        }
        FlowDir::Lr => {
            let x = from.x.max(to.x) + from.w.max(to.w) + 22;
            let y1 = from.y + from.h / 2;
            let y2 = to.y + to.h / 2;
            let x1 = from.x + from.w;
            let x2 = to.x + to.w;
            paint_orth(svg, &[(x1, y1), (x, y1), (x, y2), (x2, y2)], edge);
        }
    }
}

fn elbow_back(svg: &mut Svg, from: &Node, to: &Node, dir: FlowDir, edge: &Edge) {
    match dir {
        FlowDir::Td => {
            let x = from.x.max(to.x) + from.w.max(to.w) + 24;
            let y1 = from.y + from.h / 2;
            let y2 = to.y + to.h / 2;
            let x1 = from.x + from.w;
            let x2 = to.x + to.w;
            paint_orth(svg, &[(x1, y1), (x, y1), (x, y2), (x2, y2)], edge);
        }
        FlowDir::Lr => {
            let y = from.y.max(to.y) + from.h.max(to.h) + 24;
            let x1 = from.x + from.w / 2;
            let x2 = to.x + to.w / 2;
            let y1 = from.y + from.h;
            let y2 = to.y + to.h;
            paint_orth(svg, &[(x1, y1), (x1, y), (x2, y), (x2, y2)], edge);
        }
    }
}

fn paint_orth(svg: &mut Svg, points: &[(i32, i32)], edge: &Edge) {
    let attrs = edge_attrs(edge.kind);
    let last = points.len() - 1;
    for i in 0..last {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];
        if i + 1 == last && has_arrow(edge.kind) {
            let (sx, sy) = svg::shorten(x1, y1, x2, y2, 8);
            svg.line(x1, y1, sx, sy, attrs);
            let head = svg::arrow_head(x1, y1, x2, y2);
            svg.polygon(&head, STROKE, None);
        } else {
            svg.line(x1, y1, x2, y2, attrs);
        }
    }
    if let Some(label) = &edge.label {
        let mid = points.len() / 2;
        let (x, y) = points[mid];
        svg.text_at(x, y - 10, "middle", svg::TEXT, label);
    }
}

fn paint_line(svg: &mut Svg, x1: i32, y1: i32, x2: i32, y2: i32, edge: &Edge) {
    let attrs = edge_attrs(edge.kind);
    if has_arrow(edge.kind) {
        let (sx, sy) = svg::shorten(x1, y1, x2, y2, 8);
        svg.line(x1, y1, sx, sy, attrs);
        let head = svg::arrow_head(x1, y1, x2, y2);
        svg.polygon(&head, STROKE, None);
    } else {
        svg.line(x1, y1, x2, y2, attrs);
    }
    if let Some(label) = &edge.label {
        let mx = (x1 + x2) / 2;
        let my = (y1 + y2) / 2 - 10;
        svg.text_at(mx, my, "middle", svg::TEXT, label);
    }
}

fn draw_self_loop(svg: &mut Svg, node: &Node, edge: &Edge) {
    let x1 = node.x + node.w / 2 - 10;
    let x2 = node.x + node.w / 2 + 10;
    let y = node.y + node.h;
    let d = format!(
        "M {x1} {y} C {x1} {y_c}, {x2} {y_c}, {x2} {y}",
        y_c = y + 28
    );
    svg.path(&d, edge_attrs(edge.kind));
    if has_arrow(edge.kind) {
        svg.polygon(&[(x2, y), (x2 - 4, y + 8), (x2 + 4, y + 8)], STROKE, None);
    }
    if let Some(label) = &edge.label {
        svg.text_at(node.x + node.w / 2, y + 40, "middle", svg::TEXT, label);
    }
}

fn has_arrow(kind: EdgeKind) -> bool {
    !matches!(kind, EdgeKind::Line)
}

fn edge_attrs(kind: EdgeKind) -> &'static str {
    match kind {
        EdgeKind::Arrow | EdgeKind::Line => "stroke=\"#334155\" stroke-width=\"1.5\"",
        EdgeKind::Dotted => "stroke=\"#334155\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\"",
        EdgeKind::Thick => "stroke=\"#334155\" stroke-width=\"3\"",
    }
}
