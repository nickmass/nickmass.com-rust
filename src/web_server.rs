extern crate regex;

use hyper::server::Server;
use hyper::net::{Fresh, HttpListener};
use hyper::server::response::Response;
use hyper::server::request::Request;
use hyper::uri::RequestUri;
use hyper::status::StatusCode;
use hyper::method::Method;

use self::regex::Regex;

use error;

use std::net::ToSocketAddrs;
use std::fmt::Debug;
use std::collections::HashMap;

pub trait ContextFactory {
    type Context;
    fn get(&self) -> Self::Context;
}

pub struct WebServer<TFactory, TRoute> {
    server: Server<HttpListener>,
    factory: TFactory,
    router: Router<TRoute>,
}

impl<TFactory, TRoute> WebServer<TFactory, TRoute> where
    TFactory: 'static + ContextFactory<Context=TRoute::Context> + Sync + Send,
    TRoute: 'static + HttpHandler + Sync + Send,
{

    pub fn new<To: ToSocketAddrs + Debug>(addr: To, factory: TFactory, router: Router<TRoute>)
                                          -> WebServer<TFactory, TRoute> {

        let server = Server::http(addr).map_err(error::log).unwrap();

        WebServer {
            server: server,
            factory: factory,
            router: router,
        }
    }

    pub fn run(self) {
        info!("Starting Web Server");
        let fact = self.factory;
        let router = self.router;

        self.server.handle(move |req: Request, mut res: Response<Fresh>| {
            info!("Incoming request to: {}", req.uri);
            let route = router.route(&req);

            match route {
                Some(r) => {
                    r.exec(fact.get(), req, res);
                },
                _ => {
                    *res.status_mut() = StatusCode::NotFound;
                    res.send(b"Bad Request").ok();
                },
            };
        }).ok();;
    }
}

pub trait HttpHandler {
    type Context;
    fn exec(&self, ctx: Self::Context, req: Request, res: Response);
}

struct RouteRegex<T> {
    method: Method,
    regex: Regex,
    route: T,
}

impl<T> RouteMatch<T> for RouteRegex<T> where T: Send + Sync {
    fn route(&self) -> &T {
        &self.route
    }

    fn get_match(&self, url: &str, method: &Method) -> Option<HashMap<String, String>> {
        if &self.method != method  { return None; }
        self.regex.captures(url)
            .map(|r| r.iter_named()
                 .filter(|x| x.1.is_some())
                 .fold(HashMap::new(), |mut map, cap| {
                     map.insert(cap.0.to_owned(), cap.1.unwrap().to_owned());
                     map
        }))
    }
}

struct RouteItem<T> {
    method: Method,
    path: String,
    route: T,
}

impl<T> RouteMatch<T> for RouteItem<T> where T: Send + Sync {
    fn route(&self) -> &T {
        &self.route
    }

    fn get_match(&self, url: &str, method: &Method) -> Option<HashMap<String, String>> {
        if &self.method == method && url.starts_with(&*self.path) {
            Some(HashMap::new())
        } else {
            None
        }
    }
}

trait RouteMatch<T> : Send + Sync {
    fn route(&self) -> &T;

    fn get_match(&self, url: &str, method: &Method) -> Option<HashMap<String, String>>;
}

pub struct Router<T> {
    routes: Vec<Box<RouteMatch<T>>>,
}

impl<T> Router<T> where T: 'static + Send + Sync + HttpHandler {
    pub fn new() -> Router<T> {
        Router {
            routes: Vec::new(),
        }
    }

    pub fn add_regex(&mut self, method: Method, regex: &str, route: T) {
        self.routes.push(Box::new(RouteRegex {
            method: method,
            regex: Regex::new(regex).unwrap(),
            route: route,
        }));
    }

    pub fn add<S>(&mut self, method: Method, path: S, route: T) where
        S: ToOwned<Owned=String>, String: ::std::borrow::Borrow<S> {
        self.routes.push(Box::new(RouteItem {
            method: method,
            path: path.to_owned(),
            route: route,
        }));
    }

    fn route(&self, req: &Request) -> Option<&T> {
        let route = {
            if let RequestUri::AbsolutePath(ref url) = req.uri {
                self.routes.iter().find(|x| x.get_match(&*url, &req.method).is_some())
            } else {
                None
            }
        };

        route.as_ref().map(|x|x.route())
    }
}
