use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    println!("hibana swarm coordinator wasip1 app tick={tick}");
}
