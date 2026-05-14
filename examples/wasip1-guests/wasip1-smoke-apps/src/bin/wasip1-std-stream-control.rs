use hibana_wasi_guest::net::Stream;
use std::io::Write;

fn main() -> hibana_wasi_guest::Result<()> {
    let stream = Stream::control()?;

    let payload = b"ping";
    let written = stream.write_chunk(payload)?;
    assert_eq!(written, payload.len(), "stream write_chunk length");

    let mut recv = [0u8; Stream::MAX_CHUNK];
    let n = stream.read_chunk(&mut recv)?;
    assert_eq!(&recv[..n], b"pong", "stream read_chunk payload");

    stream.shutdown()?;

    let mut stdout = std::io::stdout();
    stdout
        .write_all(b"hibana network stream control ping pong\n")
        .expect("write stream marker");
    Ok(())
}
