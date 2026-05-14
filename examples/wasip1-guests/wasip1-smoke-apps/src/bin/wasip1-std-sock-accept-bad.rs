use hibana_wasi_guest::net::Listener;

const REJECT_MARKER: &str = "sock_accept must reject";

fn main() -> hibana_wasi_guest::Result<()> {
    let listener = Listener::control()?;
    let stream = listener.accept_stream().expect(REJECT_MARKER);
    core::hint::black_box(&stream);
    Ok(())
}
