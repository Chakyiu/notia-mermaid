use crate::error::RenderError;
use crate::limits::{self, Budget, MAX_NESTING, MAX_NODES};
use crate::scan::{self};
use crate::svg::{self, FILL, MUTED, NOTE_FILL, NOTE_STROKE, PAD, STROKE, Svg, TEXT};

const ACTOR_H: i32 = 36;
const ACTOR_GAP: i32 = 56;
const MSG_GAP: i32 = 40;
const SELF_GAP: i32 = 52;
const NOTE_GAP: i32 = 44;
const NOTE_H: i32 = 28;
const LOOP_HEAD: i32 = 26;
const LOOP_TAIL: i32 = 14;
const ALT_HEAD: i32 = 26;
const ALT_TAIL: i32 = 14;
const ELSE_GAP: i32 = 22;

pub(crate) fn render(source: &str) -> Result<Vec<u8>, RenderError> {
    let parsed = parse(source)?;
    Ok(layout(&parsed.actors, &parsed.stmts))
}

struct Actor {
    id: String,
    label: String,
    x: i32,
    w: i32,
}

#[derive(Clone, Copy)]
enum MsgKind {
    SolidArrow,
    DashedArrow,
    SolidLine,
    DashedLine,
}

enum NotePlace {
    LeftOf,
    RightOf,
    Over,
}

enum Stmt {
    Message {
        from: usize,
        to: usize,
        kind: MsgKind,
        text: String,
    },
    Note {
        place: NotePlace,
        actors: Vec<usize>,
        text: String,
    },
    Loop {
        label: String,
        body: Vec<Stmt>,
    },
    Alt {
        branches: Vec<(String, Vec<Stmt>)>,
    },
}

struct Parsed {
    actors: Vec<Actor>,
    stmts: Vec<Stmt>,
}

enum Frame {
    Root {
        body: Vec<Stmt>,
    },
    Loop {
        label: String,
        body: Vec<Stmt>,
        line: u32,
    },
    Alt {
        branches: Vec<(String, Vec<Stmt>)>,
        line: u32,
    },
}

fn parse(source: &str) -> Result<Parsed, RenderError> {
    let mut actors = Vec::new();
    let mut stack = vec![Frame::Root { body: Vec::new() }];
    let mut budget = Budget::new();
    let mut saw_header = false;
    for (idx, raw) in source.lines().enumerate() {
        let line_no = u32::try_from(idx + 1).unwrap_or(u32::MAX);
        let line = scan::strip_comment(raw).trim().trim_end_matches(';').trim();
        if line.is_empty() {
            continue;
        }
        if !saw_header {
            saw_header = true;
            if !scan::starts_with_word(line, "sequenceDiagram") {
                return Err(RenderError::invalid(
                    line_no,
                    "expected sequenceDiagram",
                    line,
                ));
            }
            continue;
        }
        budget.bump(line_no, line)?;
        if let Some(rest) = keyword_rest(line, "participant") {
            declare(&mut actors, rest, line_no, line)?;
            continue;
        }
        if let Some(rest) = keyword_rest(line, "actor") {
            declare(&mut actors, rest, line_no, line)?;
            continue;
        }
        if is_seq_reserved(line) {
            return Err(RenderError::unsupported(line_no, line));
        }
        if let Some(label) = keyword_rest(line, "loop") {
            limits::check_label(label, line_no, line)?;
            if stack.len() > MAX_NESTING {
                return Err(RenderError::limit_at(
                    line_no,
                    format!("diagram exceeds {MAX_NESTING} nested blocks"),
                    line,
                ));
            }
            stack.push(Frame::Loop {
                label: label.to_string(),
                body: Vec::new(),
                line: line_no,
            });
            continue;
        }
        if let Some(label) = keyword_rest(line, "alt") {
            limits::check_label(label, line_no, line)?;
            if stack.len() > MAX_NESTING {
                return Err(RenderError::limit_at(
                    line_no,
                    format!("diagram exceeds {MAX_NESTING} nested blocks"),
                    line,
                ));
            }
            stack.push(Frame::Alt {
                branches: vec![(label.to_string(), Vec::new())],
                line: line_no,
            });
            continue;
        }
        if let Some(label) = keyword_rest(line, "else") {
            limits::check_label(label, line_no, line)?;
            match stack.last_mut() {
                Some(Frame::Alt { branches, .. }) => {
                    branches.push((label.to_string(), Vec::new()));
                }
                _ => {
                    return Err(RenderError::invalid(line_no, "else without alt", line));
                }
            }
            continue;
        }
        if line == "end" {
            let frame = stack
                .pop()
                .ok_or_else(|| RenderError::invalid(line_no, "end without loop or alt", line))?;
            match frame {
                Frame::Root { .. } => {
                    return Err(RenderError::invalid(
                        line_no,
                        "end without loop or alt",
                        line,
                    ));
                }
                Frame::Loop { label, body, .. } => {
                    push_stmt(&mut stack, Stmt::Loop { label, body })?;
                }
                Frame::Alt { branches, .. } => {
                    push_stmt(&mut stack, Stmt::Alt { branches })?;
                }
            }
            continue;
        }
        if scan::starts_with_word(line, "Note") {
            let note = parse_note(line, line_no, &mut actors)?;
            push_stmt(&mut stack, note)?;
            continue;
        }
        if let Some(message) = parse_message(line, line_no, &mut actors)? {
            push_stmt(&mut stack, message)?;
            continue;
        }
        return Err(RenderError::invalid(
            line_no,
            "unrecognized statement",
            line,
        ));
    }
    if !saw_header {
        return Err(RenderError::empty());
    }
    if stack.len() != 1 {
        let line = match stack.last() {
            Some(Frame::Loop { line, .. } | Frame::Alt { line, .. }) => *line,
            _ => 1,
        };
        return Err(RenderError::invalid(
            line,
            "unclosed loop or alt",
            "loop or alt",
        ));
    }
    let Frame::Root { body } = stack.pop().unwrap_or(Frame::Root { body: Vec::new() }) else {
        return Err(RenderError::invalid(
            1,
            "unclosed loop or alt",
            "loop or alt",
        ));
    };
    Ok(Parsed {
        actors,
        stmts: body,
    })
}

fn keyword_rest<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(word)?;
    if rest.is_empty() {
        return Some("");
    }
    if rest.starts_with(|c: char| c.is_whitespace()) {
        return Some(rest.trim());
    }
    None
}

fn is_seq_reserved(line: &str) -> bool {
    const WORDS: &[&str] = &[
        "par",
        "and",
        "opt",
        "critical",
        "break",
        "rect",
        "autonumber",
        "box",
        "link",
        "links",
        "activate",
        "deactivate",
        "create",
        "destroy",
        "title",
    ];
    WORDS.iter().any(|word| scan::starts_with_word(line, word))
}

fn declare(
    actors: &mut Vec<Actor>,
    rest: &str,
    line_no: u32,
    stmt: &str,
) -> Result<(), RenderError> {
    if rest.is_empty() {
        return Err(RenderError::invalid(
            line_no,
            "participant is missing an id",
            stmt,
        ));
    }
    let (id, label) = if let Some((id, label)) = rest.split_once(" as ") {
        (id.trim(), label.trim())
    } else {
        (rest, rest)
    };
    if label.is_empty() {
        return Err(RenderError::invalid(
            line_no,
            "participant label is empty",
            stmt,
        ));
    }
    ensure_actor(actors, id, Some(label), line_no, stmt)?;
    Ok(())
}

fn ensure_actor(
    actors: &mut Vec<Actor>,
    id: &str,
    label: Option<&str>,
    line_no: u32,
    stmt: &str,
) -> Result<usize, RenderError> {
    if !scan::is_ident(id) {
        return Err(RenderError::invalid(line_no, "identifier expected", stmt));
    }
    if let Some(index) = actors.iter().position(|actor| actor.id == id) {
        if let Some(label) = label {
            limits::check_label(label, line_no, stmt)?;
            actors[index].label = label.to_string();
        }
        return Ok(index);
    }
    if actors.len() >= MAX_NODES {
        return Err(RenderError::limit_at(
            line_no,
            format!("diagram exceeds {MAX_NODES} nodes"),
            stmt,
        ));
    }
    let label = label.unwrap_or(id);
    limits::check_label(label, line_no, stmt)?;
    actors.push(Actor {
        id: id.to_string(),
        label: label.to_string(),
        x: 0,
        w: 0,
    });
    Ok(actors.len() - 1)
}

fn parse_note(line: &str, line_no: u32, actors: &mut Vec<Actor>) -> Result<Stmt, RenderError> {
    let Some(rest) = keyword_rest(line, "Note") else {
        return Err(RenderError::invalid(line_no, "expected a note", line));
    };
    let (place, rest) = if let Some(rest) = rest.strip_prefix("left of ") {
        (NotePlace::LeftOf, rest)
    } else if let Some(rest) = rest.strip_prefix("right of ") {
        (NotePlace::RightOf, rest)
    } else if let Some(rest) = rest.strip_prefix("over ") {
        (NotePlace::Over, rest)
    } else {
        return Err(RenderError::invalid(
            line_no,
            "note must be `left of`, `right of`, or `over`",
            line,
        ));
    };
    let Some((target, text)) = rest.split_once(':') else {
        return Err(RenderError::invalid(line_no, "note is missing ':'", line));
    };
    let text = text.trim();
    limits::check_label(text, line_no, line)?;
    let mut ids = Vec::new();
    for part in target.split(',') {
        let id = part.trim();
        if id.is_empty() {
            continue;
        }
        ids.push(ensure_actor(actors, id, None, line_no, line)?);
    }
    if ids.is_empty() {
        return Err(RenderError::invalid(
            line_no,
            "note is missing a participant",
            line,
        ));
    }
    if !matches!(place, NotePlace::Over) && ids.len() != 1 {
        return Err(RenderError::invalid(
            line_no,
            "left of / right of takes one participant",
            line,
        ));
    }
    Ok(Stmt::Note {
        place,
        actors: ids,
        text: text.to_string(),
    })
}

fn parse_message(
    line: &str,
    line_no: u32,
    actors: &mut Vec<Actor>,
) -> Result<Option<Stmt>, RenderError> {
    if let Some(hit) = find_arrow(line) {
        if hit.unsupported {
            return Err(RenderError::unsupported(line_no, line));
        }
        let from = line[..hit.start].trim();
        let after = line[hit.end..].trim();
        let (to, text) = if let Some((to, text)) = after.split_once(':') {
            (to.trim(), text.trim())
        } else {
            (after, "")
        };
        limits::check_label(text, line_no, line)?;
        let from = ensure_actor(actors, from, None, line_no, line)?;
        let to = ensure_actor(actors, to, None, line_no, line)?;
        return Ok(Some(Stmt::Message {
            from,
            to,
            kind: hit.kind,
            text: text.to_string(),
        }));
    }
    Ok(None)
}

struct ArrowHit {
    start: usize,
    end: usize,
    kind: MsgKind,
    unsupported: bool,
}

fn find_arrow(line: &str) -> Option<ArrowHit> {
    const BAD: &[&str] = &["-->>+", "-->>-", "->>+", "->>-", "--x", "-x", "--)", "-)"];
    const GOOD: &[(&str, MsgKind)] = &[
        ("-->>", MsgKind::DashedArrow),
        ("-->", MsgKind::DashedLine),
        ("->>", MsgKind::SolidArrow),
        ("->", MsgKind::SolidLine),
    ];
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'-' {
            let rest = &line[i..];
            if BAD.iter().any(|token| rest.starts_with(token)) {
                return Some(ArrowHit {
                    start: i,
                    end: i,
                    kind: MsgKind::SolidArrow,
                    unsupported: true,
                });
            }
            for (token, kind) in GOOD {
                if let Some(after) = rest.strip_prefix(token) {
                    if after.starts_with('+') || after.starts_with('-') {
                        return Some(ArrowHit {
                            start: i,
                            end: i,
                            kind: MsgKind::SolidArrow,
                            unsupported: true,
                        });
                    }
                    return Some(ArrowHit {
                        start: i,
                        end: i + token.len(),
                        kind: *kind,
                        unsupported: false,
                    });
                }
            }
        }
        i += 1;
    }
    None
}

fn push_stmt(stack: &mut [Frame], stmt: Stmt) -> Result<(), RenderError> {
    let Some(frame) = stack.last_mut() else {
        return Err(RenderError::invalid(1, "unclosed loop or alt", "end"));
    };
    let body = match frame {
        Frame::Root { body } | Frame::Loop { body, .. } => body,
        Frame::Alt { branches, .. } => {
            let Some(branch) = branches.last_mut() else {
                return Err(RenderError::invalid(1, "alt is missing a branch", "alt"));
            };
            &mut branch.1
        }
    };
    body.push(stmt);
    Ok(())
}

fn layout(actors: &[Actor], stmts: &[Stmt]) -> Vec<u8> {
    let mut actors: Vec<Actor> = actors
        .iter()
        .map(|actor| {
            let w = (svg::text_width(&actor.label) + 32).max(80);
            Actor {
                id: actor.id.clone(),
                label: actor.label.clone(),
                x: 0,
                w,
            }
        })
        .collect();
    let mut cursor = 0;
    for actor in &mut actors {
        actor.x = cursor;
        cursor += actor.w + ACTOR_GAP;
    }
    let content_right = if actors.is_empty() {
        120
    } else {
        cursor - ACTOR_GAP
    };
    let left_pad = extra_left(stmts, &actors).max(0);
    let shift = PAD + left_pad;
    for actor in &mut actors {
        actor.x += shift;
    }
    let right = extra_right(stmts, &actors).max(content_right + shift);
    let width = right + PAD;
    let top = PAD + ACTOR_H + 16;
    let height = top + block_height(stmts) + PAD;

    let mut svg = Svg::new();
    let bottom = height - PAD;
    for actor in &actors {
        let cx = actor.x + actor.w / 2;
        svg.line(
            cx,
            PAD + ACTOR_H,
            cx,
            bottom,
            "stroke=\"#94a3b8\" stroke-width=\"1.5\" stroke-dasharray=\"2 3\"",
        );
    }
    draw_block(&mut svg, stmts, &actors, top, width);
    for actor in &actors {
        svg.rect(actor.x, PAD, actor.w, ACTOR_H, 6, FILL, STROKE, None);
        svg.text_at(
            actor.x + actor.w / 2,
            PAD + ACTOR_H / 2,
            "middle",
            TEXT,
            &actor.label,
        );
    }
    svg.finish(width, height)
}

fn block_height(stmts: &[Stmt]) -> i32 {
    stmts.iter().map(stmt_height).sum()
}

fn stmt_height(stmt: &Stmt) -> i32 {
    match stmt {
        Stmt::Message { from, to, .. } => {
            if from == to {
                SELF_GAP
            } else {
                MSG_GAP
            }
        }
        Stmt::Note { .. } => NOTE_GAP,
        Stmt::Loop { body, .. } => LOOP_HEAD + block_height(body) + LOOP_TAIL,
        Stmt::Alt { branches, .. } => {
            let mut height = ALT_HEAD + ALT_TAIL;
            for (i, (_, body)) in branches.iter().enumerate() {
                if i > 0 {
                    height += ELSE_GAP;
                }
                height += block_height(body);
            }
            height
        }
    }
}

fn draw_block(svg: &mut Svg, stmts: &[Stmt], actors: &[Actor], mut y: i32, width: i32) -> i32 {
    for stmt in stmts {
        y = draw_stmt(svg, stmt, actors, y, width);
    }
    y
}

fn draw_stmt(svg: &mut Svg, stmt: &Stmt, actors: &[Actor], y: i32, width: i32) -> i32 {
    let height = stmt_height(stmt);
    match stmt {
        Stmt::Message {
            from,
            to,
            kind,
            text,
        } => {
            draw_message(svg, actors, *from, *to, kind, text, y);
        }
        Stmt::Note {
            place,
            actors: ids,
            text,
        } => {
            draw_note(svg, actors, place, ids, text, y);
        }
        Stmt::Loop { label, body } => {
            svg.rect(
                PAD,
                y,
                width - 2 * PAD,
                height,
                6,
                "#f8fafc",
                MUTED,
                Some("4 3"),
            );
            svg.text_at(PAD + 8, y + 14, "start", TEXT, label);
            draw_block(svg, body, actors, y + LOOP_HEAD, width);
        }
        Stmt::Alt { branches } => {
            svg.rect(
                PAD,
                y,
                width - 2 * PAD,
                height,
                6,
                "#f8fafc",
                MUTED,
                Some("4 3"),
            );
            let mut cursor = y + ALT_HEAD;
            for (i, (label, body)) in branches.iter().enumerate() {
                if i == 0 {
                    svg.text_at(PAD + 8, y + 14, "start", TEXT, label);
                } else {
                    svg.line(
                        PAD,
                        cursor,
                        width - PAD,
                        cursor,
                        "stroke=\"#94a3b8\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\"",
                    );
                    svg.text_at(PAD + 8, cursor + 11, "start", TEXT, label);
                    cursor += ELSE_GAP;
                }
                cursor = draw_block(svg, body, actors, cursor, width);
            }
        }
    }
    y + height
}

fn draw_message(
    svg: &mut Svg,
    actors: &[Actor],
    from: usize,
    to: usize,
    kind: &MsgKind,
    text: &str,
    y: i32,
) {
    let attrs = msg_attrs(kind);
    let arrow = msg_has_arrow(kind);
    if from == to {
        let actor = &actors[from];
        let x = actor.x + actor.w / 2;
        let y1 = y + 10;
        let y2 = y + 34;
        let d = format!("M {x} {y1} C {xr} {y1}, {xr} {y2}, {x} {y2}", xr = x + 28);
        svg.path(&d, attrs);
        if arrow {
            svg.polygon(&[(x, y2), (x + 8, y2 - 4), (x + 8, y2 + 4)], STROKE, None);
        }
        if !text.is_empty() {
            svg.text_at(x + 36, (y1 + y2) / 2, "start", TEXT, text);
        }
        return;
    }
    let x1 = actors[from].x + actors[from].w / 2;
    let x2 = actors[to].x + actors[to].w / 2;
    let y_line = y + 26;
    if arrow {
        let (sx, sy) = svg::shorten(x1, y_line, x2, y_line, 8);
        svg.line(x1, y_line, sx, sy, attrs);
        let head = svg::arrow_head(x1, y_line, x2, y_line);
        svg.polygon(&head, STROKE, None);
    } else {
        svg.line(x1, y_line, x2, y_line, attrs);
    }
    if !text.is_empty() {
        svg.text_at((x1 + x2) / 2, y_line - 10, "middle", TEXT, text);
    }
}

fn draw_note(
    svg: &mut Svg,
    actors: &[Actor],
    place: &NotePlace,
    ids: &[usize],
    text: &str,
    y: i32,
) {
    let (x, w) = note_box(actors, place, ids, text);
    let yb = y + 8;
    svg.rect(x, yb, w, NOTE_H, 4, NOTE_FILL, NOTE_STROKE, None);
    svg.text_at(x + w / 2, yb + NOTE_H / 2, "middle", TEXT, text);
}

fn note_width(text: &str) -> i32 {
    (svg::text_width(text) + 20).max(48)
}

fn note_box(actors: &[Actor], place: &NotePlace, ids: &[usize], text: &str) -> (i32, i32) {
    let note_w = note_width(text);
    match place {
        NotePlace::LeftOf => {
            let actor = &actors[ids[0]];
            (actor.x - 12 - note_w, note_w)
        }
        NotePlace::RightOf => {
            let actor = &actors[ids[0]];
            (actor.x + actor.w + 12, note_w)
        }
        NotePlace::Over => over_box(actors, ids, note_w),
    }
}

fn over_box(actors: &[Actor], ids: &[usize], note_w: i32) -> (i32, i32) {
    let mut left = i32::MAX;
    let mut right = i32::MIN;
    for &id in ids {
        left = left.min(actors[id].x);
        right = right.max(actors[id].x + actors[id].w);
    }
    if left == i32::MAX {
        return (PAD, note_w);
    }
    let span = right - left;
    if note_w > span {
        (left - (note_w - span) / 2, note_w)
    } else {
        (left, span)
    }
}

fn extra_left(stmts: &[Stmt], actors: &[Actor]) -> i32 {
    let mut need = 0;
    walk(stmts, &mut |stmt| {
        if let Stmt::Note {
            place,
            actors: ids,
            text,
        } = stmt
        {
            let (x, _) = note_box(actors, place, ids, text);
            if x < 0 {
                need = need.max(-x);
            }
        }
    });
    need
}

fn extra_right(stmts: &[Stmt], actors: &[Actor]) -> i32 {
    let mut need = 0;
    walk(stmts, &mut |stmt| match stmt {
        Stmt::Note {
            place,
            actors: ids,
            text,
        } => {
            let (x, w) = note_box(actors, place, ids, text);
            need = need.max(x + w);
        }
        Stmt::Message { from, to, text, .. } if from == to && !text.is_empty() => {
            let actor = &actors[*from];
            let cx = actor.x + actor.w / 2;
            need = need.max(cx + 36 + svg::text_width(text) + 8);
        }
        _ => {}
    });
    need
}

fn walk(stmts: &[Stmt], visit: &mut dyn FnMut(&Stmt)) {
    for stmt in stmts {
        visit(stmt);
        match stmt {
            Stmt::Loop { body, .. } => walk(body, visit),
            Stmt::Alt { branches, .. } => {
                for (_, body) in branches {
                    walk(body, visit);
                }
            }
            _ => {}
        }
    }
}

fn msg_has_arrow(kind: &MsgKind) -> bool {
    matches!(kind, MsgKind::SolidArrow | MsgKind::DashedArrow)
}

fn msg_attrs(kind: &MsgKind) -> &'static str {
    match kind {
        MsgKind::SolidArrow | MsgKind::SolidLine => "stroke=\"#334155\" stroke-width=\"1.5\"",
        MsgKind::DashedArrow | MsgKind::DashedLine => {
            "stroke=\"#334155\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\""
        }
    }
}
