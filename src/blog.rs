use std::collections::HashMap;

use posts::{Post, PostService};
use web_server::{ContextFactory, HttpHandler, Router, RouteParams};

use hyper::server::response::Response;
use hyper::server::request::Request;

use regex::Regex;

use r2d2;
use r2d2_redis::RedisConnectionManager;

pub struct BlogContext {
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
        router.get(Regex::new("^/api/posts/(?P<id>[:digit:]{1,10})/?$").unwrap(),
                                     Route::GetPost);
        router.get(Regex::new("^/api/posts/(?P<fragment>.+)/?$").unwrap(), Route::GetPostByFragment);
        router.get(Regex::new("^/api/posts/?$").unwrap(), Route::GetPosts);
        router.post(Regex::new("^/api/posts/?$").unwrap(), Route::CreatePost);
        router.put(Regex::new("^/api/posts/?$").unwrap(), Route::UpdatePost);
        router.delete(Regex::new("^/api/posts/(?P<id>[:digit:]+)/?$").unwrap(), Route::DeletePost);
        router
    }
}

impl HttpHandler for Route {
    type Context = BlogContext;

    fn exec(&self, ctx: Self::Context, req: Request, res: Response, params: RouteParams) {
        info!("Matched Route {:?}", self);
        let posts = PostService::new(ctx.redis.get().unwrap());
        match *self {
            Route::GetPosts => {
                posts.list(10, 0);
            },
            Route::GetPost => {
                posts.get(1);
            },
            Route::GetPostByFragment => {
                posts.get_by_fragment("asd");
            }
            _ => {}
        }
        res.send(b"Hello World").ok();
    }
}
