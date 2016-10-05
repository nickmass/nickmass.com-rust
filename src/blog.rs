extern crate url;
extern crate serde_json;

use std::collections::HashMap;

use posts::{Post, PostService};
use web_server::{ContextFactory, HttpHandler, Router, RouteParams};

use hyper::server::response::Response;
use hyper::server::request::Request;
use hyper::uri::RequestUri;

use regex::Regex;
use self::url::Url;

use r2d2;
use r2d2_redis::RedisConnectionManager;

pub struct BlogContext {
    posts: PostService,
    redis: r2d2::Pool<RedisConnectionManager>,
}

pub struct BlogContextFactory {
    redis: r2d2::Pool<RedisConnectionManager>,
}

impl BlogContextFactory {
    pub fn new(redis: r2d2::Pool<RedisConnectionManager>) -> BlogContextFactory {
        BlogContextFactory {
            redis: redis,
        }
    }
}

impl ContextFactory for BlogContextFactory {
    type Context = BlogContext;

    fn get(&self) -> BlogContext {
        BlogContext {
            posts: PostService::new(self.redis.clone()),
            redis: self.redis.clone()
        }
    }
}

#[derive(Copy, Debug, Clone)]
pub enum Route {
    GetPosts,
    GetPost,
    GetPostByFragment,
    CreatePost,
    UpdatePost,
    DeletePost,
}

impl Route {
    pub fn router() -> Router<Route> {
        let mut router = Router::new();
        router.get(Regex::new("^/api/posts/(?P<id>[:digit:]{1,10})/?$").unwrap(), Route::GetPost);
        router.get(Regex::new("^/api/posts/(?P<fragment>.+)/?$").unwrap(),
                   Route::GetPostByFragment);
        router.get(Regex::new("^/api/posts").unwrap(), Route::GetPosts);
        router.post(Regex::new("^/api/posts/?$").unwrap(), Route::CreatePost);
        router.put(Regex::new("^/api/posts/?$").unwrap(), Route::UpdatePost);
        router.delete(Regex::new("^/api/posts/(?P<id>[:digit:]+)/?$").unwrap(), Route::DeletePost);
        router
    }
}

impl HttpHandler for Route {
    type Context = BlogContext;

    fn exec(&self, ctx: BlogContext, req: Request, res: Response, params: RouteParams) {
        info!("Matched Route {:?}", self);

        let url = match req.uri {
            RequestUri::AbsolutePath(s) => Some(s),
            _ => None
        };

        let parser = Url::parse("http://localhost").unwrap();
        let url = url.and_then(|x| parser.join(&*x).ok());
        let url = url.expect("Valid Url");

        match *self {
            Route::GetPosts => {
                let limit = url.query_pairs()
                    .find(|x| x.0 == "limit")
                    .and_then(|x| x.1.parse().ok())
                    .unwrap_or(10);
                let skip = url.query_pairs()
                    .find(|x| x.0 == "skip")
                    .and_then(|x| x.1.parse().ok())
                    .unwrap_or(0);

                let posts = ctx.posts.list(limit, skip);

                let json = serde_json::to_vec(&posts).unwrap();
                res.send(&*json).ok();
            },
            Route::GetPost => {
                let post = ctx.posts.get(params.get("id").unwrap().parse().unwrap());

                let json = serde_json::to_vec(&post).unwrap();
                res.send(&*json).ok();
            },
            Route::GetPostByFragment => {
                let post = ctx.posts.get_by_fragment(params.get("fragment").unwrap());

                let json = serde_json::to_vec(&post).unwrap();
                res.send(&*json).ok();
            }
            _ => {
                res.send(b"Hello World").ok();
            },
        }
    }
}
