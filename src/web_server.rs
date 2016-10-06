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
use std::io::Read;

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

pub struct WebServer<TFactory> where TFactory: ContextFactory {
    server: Server<HttpListener>,
    factory: TFactory,
    middleware: Vec<Box<Middleware<Context=TFactory::Context>>>,
}

impl<TFactory> WebServer<TFactory> where
    TFactory: 'static + ContextFactory + Sync + Send,
{
    pub fn new<To>(addr: To, factory: TFactory) -> WebServer<TFactory>
        where To: ToSocketAddrs {
        let server = Server::http(addr).map_err(error::log).unwrap();

        WebServer {
            server: server,
            factory: factory,
            middleware: Vec::new(),
        }
    }

    pub fn middleware<T>(&mut self, middleware: T)
        where T: 'static + Middleware<Context=TFactory::Context> + Send + Sync {
        self.middleware.push(Box::new(middleware));
    }

    pub fn run(self) {
        info!("Starting Web Server");
        let fact = self.factory;
        let middleware = self.middleware;

        self.server.handle(move |req: HyperRequest, res: HyperResponse<Fresh>| {
            let (req, mut res) = (req.to_request(), res.to_response());
            if req.is_none() {
                res.text("Bad Request");
                return;
            }
            let mut req = req.unwrap();
            let mut ctx = fact.get();

            for ware in middleware.iter() {
                let r = ware.exec(&mut ctx, req, res);
                req = r.0;
                res = r.1;

                match res {
                    Response::Done => break,
                    _ => (),
                }
            }
        }).ok();;
    }
}

trait ToRequest<'a, 'b: 'a> {
    fn to_request(self) -> Option<Request<'a, 'b>>;
}

impl<'a, 'b: 'a> ToRequest<'a, 'b> for HyperRequest<'a, 'b> {
    fn to_request(self) -> Option<Request<'a, 'b>> {
        self.url().map(|u| Request::Headers(u, self))
    }
}

trait GetUrl {
    fn url(&self) -> Option<url::Url>;
}

impl<'a, 'b: 'a> GetUrl for HyperRequest<'a, 'b> {
    fn url(&self) -> Option<url::Url> { 
        let url = match self.uri {
            RequestUri::AbsolutePath(ref s) => s,
            _ => unimplemented!(),
        };

        let parser = url::Url::parse("http://localhost").unwrap();
        parser.join(&*url).ok()
    }
}

pub enum Request<'a, 'b: 'a> {
    Headers(url::Url, HyperRequest<'a, 'b>),
    Body(url::Url, HyperRequest<'a, 'b>, Vec<u8>),
}
impl<'a, 'b: 'a> Request<'a, 'b> {
    pub fn url(&self) -> &url::Url {
        match *self {
            Request::Headers(ref u, _) | Request::Body(ref u, _, _) => u,
        }
    }

    pub fn method(&self) -> &Method {
        match *self {
            Request::Headers(_, ref r) | Request::Body(_, ref r, _) => &r.method,
        }
    }

    pub fn query_param(&self, key: &str) -> Option<String> {
        self.url()
            .query_pairs()
            .find(|x| x.0 == key)
            .map(|x| x.1.into_owned())
    }

    pub fn body(self) -> Request<'a, 'b> {
        match self {
            Request::Headers(u, mut r) => {
                let mut buf = Vec::new();
                r.read_to_end(&mut buf);
                Request::Body(u, r, buf)
            },
            Request::Body(_, _, _) => {
                self
            },
        }
    }

    pub fn to_json<T>(&mut self) -> Result<T, serde_json::error::Error> where T: Deserialize {
        match *self {
            Request::Headers(_, _) => {
                panic!("Attempted to read body on fresh request")
            },
            Request::Body(_, _, ref b) => {
                serde_json::from_slice(b)
            },
        }
    }
}

trait ToResponse<'a> {
    fn to_response(self) -> Response<'a>;
}
impl<'a> ToResponse<'a> for HyperResponse<'a> {
    fn to_response(self) -> Response<'a> {
        Response::Fresh(self)
    }
}

pub enum Response<'a> {
    Fresh(HyperResponse<'a>),
    Headers(HyperResponse<'a>),
    Done
}

impl<'a> Response<'a> {
    pub fn json<T>(self, data: &T) -> Response<'a> where T: Serialize  {
        let result = match self {
            Response::Fresh(r) | Response::Headers(r) => {
                r.send(&*serde_json::to_vec(data).unwrap()).ok()
            },
            Response::Done => None,
        };
        Response::Done
    }

    pub fn text<T>(self, data: T) -> Response<'a> where T: AsRef<str> {
        let result = match self {
            Response::Fresh(r) | Response::Headers(r) => {
                r.send(&*data.as_ref().bytes().collect::<Vec<u8>>()).ok()
            },
            Response::Done => None,
        };
        Response::Done
    }
}

pub trait Middleware: Send + Sync {
    type Context;
    fn exec<'a, 'b: 'a, 'c>(&self, ctx: &mut Self::Context, req: Request<'a, 'b>, res: Response<'c>)
                             -> (Request<'a, 'b>, Response<'c>);
}

pub struct LogMiddleware<C> {
    _phantom: ::std::marker::PhantomData<C>,
}

impl<C> LogMiddleware<C> {
    pub fn new() -> LogMiddleware<C> {
        LogMiddleware {
            _phantom: ::std::marker::PhantomData,
        }
    }
}

impl<C> Middleware for LogMiddleware<C> where C: Send + Sync {
    type Context = C;
    fn exec<'a, 'b: 'a, 'c>(&self, ctx: &mut Self::Context, req: Request<'a, 'b>, res: Response<'c>)
                            -> (Request<'a, 'b>, Response<'c>) {
        info!("{} {}",req.method(), req.url());
        (req, res)
    }
}

pub trait RouteHandler {
    type Context;
    fn route<'a, 'b: 'a, 'c>(&self, ctx: &mut Self::Context, req: Request<'a, 'b>, res: Response<'c>,
                         params: RouteParams) -> (Request<'a, 'b>, Response<'c>);
}

pub struct Router<T> {
    routes: Vec<RouteMatch<T>>,
}

impl<T> Router<T> where T: 'static + Send + Sync + RouteHandler {
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

    fn route(&self, req: &Request) -> Option<(&T, RouteParams)> {
        self.routes.iter()
            .map(|x| x.get_match(&req.method(), req.url().path()).map(|y|(&x.route,y)))
            .find(|x| x.is_some())
            .map(|x| x.unwrap())
    }
}

impl<T> Middleware for Router<T> where T: 'static + Send + Sync + RouteHandler {
    type Context = T::Context;
    fn exec<'a, 'b: 'a, 'c>(&self, ctx: &mut Self::Context, req: Request<'a, 'b>, res: Response<'c>)
                            -> (Request<'a, 'b>, Response<'c>) {
        let route = self.route(&req);

        match route {
            Some((r, params)) => {
                r.route(ctx, req, res, params)
            },
            _ => {
                (req, res.text("Bad Request"))
            },
        }
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
