pub struct WsServer;

use std::net::ToSocketAddrs;

impl WsServer {
    pub fn new<To: ToSocketAddrs>(addr: To) -> WsServer {
        WsServer
    }

    pub fn run(self) { loop {
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    } }
}
