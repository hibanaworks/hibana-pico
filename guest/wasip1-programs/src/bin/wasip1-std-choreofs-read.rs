use std::{
    fs::File,
    io::{Read, Write},
};

fn main() {
    let mut file = File::open("config.txt").expect("open choreofs config");
    let mut value = [0u8; 16];
    let len = file.read(&mut value).expect("read choreofs config");
    let mut stdout = std::io::stdout();
    stdout
        .write_all(b"hibana choreofs read ")
        .expect("write choreofs marker");
    stdout
        .write_all(&value[..len])
        .expect("write choreofs value");
    stdout.write_all(b"\n").expect("write choreofs newline");
}
