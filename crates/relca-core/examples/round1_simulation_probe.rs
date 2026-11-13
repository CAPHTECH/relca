use relca_core::{engine, lower, reference, syntax};
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let m = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    println!(
        "{}",
        serde_json::json!({"engine":engine::simulate(&m),"reference":reference::simulate(&m).outcome})
    );
}
