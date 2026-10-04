//! The phone camera on its own, without the rest of djmanzo: for trying it
//! with a phone, and for the browser test that drives both of its pages.
//!
//! ```text
//! cargo run -p dj-net --example phone_camera -- [port] [phone key] [OBS key]
//! ```
//!
//! Port 0 asks the system for one. It prints where it listens, the two
//! addresses, and then what each side is doing whenever that changes. The
//! keys default to `phone` and `obs`, which is fine for trying it at home
//! and nowhere else: anyone on the network who guesses them can send a
//! picture. djmanzo itself makes random ones.

use dj_net::authority::Authority;
use dj_net::phone::{Keys, serve};
use std::io::Write as _;
use std::net::SocketAddr;
use std::sync::Arc;

fn main() {
    let mut args = std::env::args().skip(1);
    let port: u16 = args
        .next()
        .and_then(|p| p.parse().ok())
        .unwrap_or(dj_net::phone::DEFAULT_PORT);
    let keys = Keys {
        phone: args.next().unwrap_or_else(|| "phone".to_owned()),
        viewer: args.next().unwrap_or_else(|| "obs".to_owned()),
    };
    let authority =
        Arc::new(Authority::new("djmanzo phone camera (example)").expect("an authority"));
    let fingerprint = authority.fingerprint();
    let serving = serve(
        SocketAddr::from(([0, 0, 0, 0], port)),
        authority,
        keys.clone(),
    )
    .expect("a port to listen on");
    let port = serving.address().port();
    let host =
        dj_net::sticker::lan_address().map_or_else(|| "127.0.0.1".to_owned(), |a| a.to_string());
    println!("listening {port}");
    println!("phone: https://{host}:{port}/p/{}/", keys.phone);
    println!("OBS:   http://127.0.0.1:{port}/v/{}/", keys.viewer);
    println!("authority: {fingerprint}");
    let _ = std::io::stdout().flush();
    let mut last = None;
    loop {
        let seen = serving.seen();
        if last != Some(seen) {
            println!("phone {:?}, OBS {:?}", seen.phone, seen.viewer);
            let _ = std::io::stdout().flush();
            last = Some(seen);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
