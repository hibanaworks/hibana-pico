use hibana_wasi_guest::net::Listener;

const REJECT_MARKER: &str = "sock_accept must reject";

fn main() -> hibana_wasi_guest::Result<()> {
    let listener = Listener::control()?;
    let _stream = listener.accept_stream().expect(REJECT_MARKER);
    Ok(())
}
