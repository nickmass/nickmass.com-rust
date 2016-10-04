#![feature(plugin, custom_derive)]
#![plugin(serde_macros)]

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate dotenv;
extern crate redis;
extern crate r2d2;
extern crate r2d2_redis;
extern crate hyper;

use dotenv::dotenv;
use r2d2_redis::RedisConnectionManager;

use std::thread;
use std::env::var;

mod error;
mod web_server;
mod ws_server;
mod posts;
mod blog;

use blog::{BlogContextFactory, Route};
use web_server::WebServer;
use ws_server::WsServer;

fn main() {
    dotenv().ok();
    env_logger::init().unwrap();

    info!("Starting Application");

    let pool_config = Default::default();
    let manager = RedisConnectionManager::new(&*var("REDIS_SOCKET").unwrap()).unwrap();
    let web_pool = r2d2::Pool::new(pool_config, manager).unwrap();
    let ws_pool = web_pool.clone();

    let fact = BlogContextFactory::new(web_pool);
    let web = WebServer::new(&*var("WEB_SOCKET").unwrap(), fact, Route::router()); 
    let ws = WsServer::new(&*var("WS_SOCKET").unwrap());

    let web_t = thread::spawn(move || web.run());
    let _ws_t = thread::spawn(move || ws.run());

    web_t.join().ok();
}
