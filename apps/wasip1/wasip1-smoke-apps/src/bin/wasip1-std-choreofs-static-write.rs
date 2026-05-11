use std::{fs::OpenOptions, io::Write};

fn main() {
    let mut file = OpenOptions::new()
        .write(true)
        .open("readonly.txt")
        .expect("bad app expects read-only object open success");
    file.write_all(b"bad")
        .expect("readonly static write must reject before bad app observes success");
}
