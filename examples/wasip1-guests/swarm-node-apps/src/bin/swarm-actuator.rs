use std::io::{self, Read};

fn main() {
    let mut command = [0u8; 8];
    let bytes_read = io::stdin()
        .read(&mut command)
        .expect("read actuator command");
    core::hint::black_box(bytes_read);
    println!("hibana swarm actuator wasip1 app ack={}", command[0]);
}
