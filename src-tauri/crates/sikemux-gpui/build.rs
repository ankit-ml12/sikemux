use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/faces");
    let mut shapes: Vec<String> = std::fs::read_dir("assets/faces")
        .expect("assets/faces")
        .map(|entry| entry.expect("a face file").file_name().into_string().expect("a file name"))
        .filter(|name| name.ends_with(".json.gz"))
        .collect();
    shapes.sort();
    let entries: String = shapes
        .iter()
        .map(|file| {
            let shape = file.trim_end_matches(".json.gz");
            format!("    ({shape:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/faces/{file}\"))),\n")
        })
        .collect();
    let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("faces.rs");
    std::fs::write(out, format!("static DRAWINGS: &[(&str, &[u8])] = &[\n{entries}];\n")).expect("write faces.rs");
}
