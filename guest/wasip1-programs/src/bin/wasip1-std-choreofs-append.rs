use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
};

fn main() {
    let mut log = OpenOptions::new()
        .append(true)
        .open("log.txt")
        .expect("open choreofs append log");
    log.write_all(b"entry").expect("append choreofs log");
    drop(log);

    let mut log = File::open("log.txt").expect("open choreofs log for readback");
    let mut value = [0u8; 16];
    let len = log.read(&mut value).expect("read choreofs log");
    let mut stdout = std::io::stdout();
    stdout
        .write_all(b"hibana choreofs append ")
        .expect("write append marker");
    stdout.write_all(&value[..len]).expect("write append value");
    stdout.write_all(b"\n").expect("write append newline");
}
