pub struct WsServer;

use std::net::ToSocketAddrs;

impl WsServer {
    pub fn new<To: ToSocketAddrs>(addr: To) -> WsServer {
        WsServer
    }

    pub fn run(self) { loop {} }
}
