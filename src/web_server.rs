extern crate url;
extern crate serde;
extern crate serde_json;
extern crate hyper;
extern crate regex;

use self::hyper::net::{Fresh, HttpListener};
use self::hyper::server::{Server, Request as HyperRequest, Response as HyperResponse};
use self::hyper::uri::RequestUri;
use self::hyper::status::StatusCode;
use self::hyper::method::Method;

use self::regex::Regex;

use self::serde::Serialize;
use self::serde::Deserialize;

use error;

use std::net::ToSocketAddrs;
use std::fmt::Debug;
use std::collections::HashMap;

pub trait ContextFactory {
    type Context;
    fn get(&self) -> Self::Context;
}

impl<T, C> ContextFactory for T where T: Fn() -> C {
    type Context = C;

    fn get(&self) -> C {
        self()
    }
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

    pub fn new<To: ToSocketAddrs + Debug>(addr: To, router: Router<TRoute>, factory: TFactory)
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

        self.server.handle(move |req: HyperRequest, mut res: HyperResponse<Fresh>| {
            info!("Incoming request to: {}", req.uri);
            let route = router.route(&req);

            match route {
                Some((r, params)) => {
                    r.exec(fact.get(), Request(req), Response(res), params);
                },
                _ => {
                    *res.status_mut() = StatusCode::NotFound;
                    res.send(b"Bad Request").ok();
                },
            };
        }).ok();;
    }
}

pub struct Request<'a, 'b: 'a>(HyperRequest<'a, 'b>);
impl<'a, 'b: 'a> Request<'a, 'b> {
    pub fn url(&self) -> Option<url::Url> {
        let url = match self.0.uri {
            RequestUri::AbsolutePath(ref s) => s,
            _ => unimplemented!()
        };

        let parser = url::Url::parse("http://localhost").unwrap();
        let url = parser.join(&*url).ok();

        url
    }

    pub fn query_param(&self, key: &str) -> Option<String> {
        self.url()
            .and_then(|x| x.query_pairs()
                      .find(|x| x.0 == key)
                      .map(|x| x.1.into_owned())
            )
    }

    pub fn as_json<T>(&mut self) -> Result<T, serde_json::error::Error> where T: Deserialize {
        serde_json::from_reader(&mut self.0)
    }
}

pub struct Response<'a>(HyperResponse<'a>);
impl<'a> Response<'a> {
    pub fn json<T>(self, data: &T) where T: Serialize {
        self.0.send(&*serde_json::to_vec(data).unwrap()).ok();
    }

    pub fn text<T>(self, data: T) where T: AsRef<str> {
        self.0.send(&*data.as_ref().bytes().collect::<Vec<u8>>()).ok();
    }
}

pub trait HttpHandler {
    type Context;
    fn exec(&self, ctx: Self::Context, req: Request, res: Response, params: RouteParams);
}

pub struct Router<T> {
    routes: Vec<RouteMatch<T>>,
}

impl<T> Router<T> where T: 'static + Send + Sync + HttpHandler {
    pub fn new() -> Router<T> {
        Router {
            routes: Vec::new(),
        }
    }

    pub fn get<M>(&mut self, matcher: M, route: T)
        where M: 'static + IntoRouteMatcher {
        self.add(Method::Get, matcher, route)
    }

    pub fn post<M>(&mut self, matcher: M, route: T)
        where M: 'static + IntoRouteMatcher {
        self.add(Method::Post, matcher, route)
    }

    pub fn put<M>(&mut self, matcher: M, route: T)
        where M: 'static + IntoRouteMatcher {
        self.add(Method::Put, matcher, route)
    }

    pub fn delete<M>(&mut self, matcher: M, route: T)
        where M: 'static + IntoRouteMatcher {
        self.add(Method::Delete, matcher, route)
    }

    pub fn add<M>(&mut self, method: Method, matcher: M, route: T)
        where M: 'static + IntoRouteMatcher {
        self.routes.push(RouteMatch {
            matcher: Box::new(matcher.into_matcher()),
            route: route,
            method: method,
        });
    }

    fn route(&self, req: &HyperRequest) -> Option<(&T, RouteParams)> {
        let route = {
            if let RequestUri::AbsolutePath(ref url) = req.uri {
                self.routes.iter()
                    .map(|x| x.get_match(&req.method, &*url).map(|y|(&x.route,y)))
                    .find(|x| x.is_some())
                    .map(|x| x.unwrap())
            } else {
                None
            }
        };

        route
    }
}

pub struct RouteParams {
    params: HashMap<String, String>,
}

impl RouteParams {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(|x| &**x)
    }
}

pub trait RouteMatcher: Send + Sync {
    fn get_match(&self, url: &str) -> Option<RouteParams>;
}

pub struct RegexMatcher {
    regex: Regex,
}

impl RouteMatcher for RegexMatcher {
    fn get_match(&self, url: &str) -> Option<RouteParams> {
        self.regex.captures(url)
            .map(|r| r.iter_named()
                 .filter(|x| x.1.is_some())
                 .fold(HashMap::new(), |mut map, cap| {
                     map.insert(cap.0.to_owned(), cap.1.unwrap().to_owned());
                     map
                 }))
            .map(|x| RouteParams{ params: x})
    }
}

pub struct DefaultMatcher {
    pattern: String
}

impl RouteMatcher for DefaultMatcher {
    fn get_match(&self, url: &str) -> Option<RouteParams> {
        if url.starts_with(&*self.pattern) {
            Some(RouteParams{ params: HashMap::new() })
        } else {
            None
        }
    }
}

struct RouteMatch<T> {
    matcher: Box<RouteMatcher>,
    route: T,
    method: Method,
}

impl<T> RouteMatch<T> {
    fn get_match(&self, method: &Method, url: &str) -> Option<RouteParams> {
        if method != &self.method { return None; }
        self.matcher.get_match(url)
    }
}

pub trait IntoRouteMatcher {
    type Matcher: RouteMatcher;
    fn into_matcher(self) -> Self::Matcher;
}

impl IntoRouteMatcher for Regex {
    type Matcher = RegexMatcher;

    fn into_matcher(self) -> RegexMatcher {
        RegexMatcher { regex: self }
    }
}

impl<'a> IntoRouteMatcher for &'a str {
    type Matcher =  DefaultMatcher;

    fn into_matcher(self) -> DefaultMatcher {
        DefaultMatcher { pattern: self.to_owned() }
    }
}

impl IntoRouteMatcher for String {
    type Matcher = DefaultMatcher;

    fn into_matcher(self) -> DefaultMatcher {
        DefaultMatcher { pattern: self }
    }
}
