fn main() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
    for name in ["flowchart", "sequence"] {
        let src = std::fs::read_to_string(root.join(format!("{name}.mmd")))
            .unwrap_or_else(|err| panic!("read {name}.mmd: {err}"));
        let svg = notia_mermaid::render(&src).unwrap_or_else(|err| panic!("{name}: {err}"));
        let path = root.join(format!("{name}.svg"));
        std::fs::write(&path, &svg).unwrap_or_else(|err| panic!("write {}: {err}", path.display()));
        println!("wrote {}", path.display());
    }
}
