use hibana_wasi_guest::net::{Listener, Stream};
use std::io::Write;

fn main() -> hibana_wasi_guest::Result<()> {
    let listener = Listener::control()?;
    let stream = listener.accept_stream()?;

    let payload = b"ping";
    let written = stream.write_chunk(payload)?;
    assert_eq!(written, payload.len(), "accepted sock_send length");

    let mut recv = [0u8; Stream::MAX_CHUNK];
    let nread = stream.read_chunk(&mut recv)?;
    assert_eq!(&recv[..nread], b"pong", "accepted sock_recv payload");

    stream.shutdown()?;

    let mut stdout = std::io::stdout();
    stdout
        .write_all(b"hibana listener accept fd ping pong\n")
        .expect("write accept marker");
    Ok(())
}
