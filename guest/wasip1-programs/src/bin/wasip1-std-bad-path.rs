use std::fs;

fn main() {
    let contents = fs::read_to_string("forbidden.txt")
        .expect("forbidden path must reject before bad app observes success");
    core::hint::black_box(contents.len());
}
