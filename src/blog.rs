use posts::{Post, PostService};
use web_server::{ContextFactory, HttpHandler, Router};
use hyper::server::response::Response;
use hyper::server::request::Request;
use hyper::method::Method;

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
        router.add_regex(Method::Get, "^/api/posts/(?P<id>[:digit:]{1,10})/?$", Route::GetPost);
        router.add_regex(Method::Get, "^/api/posts/(?P<fragment>.+)/?$", Route::GetPostByFragment);
        router.add_regex(Method::Get, "^/api/posts/?$", Route::GetPosts);
        router.add_regex(Method::Post, "^/api/posts/?$", Route::CreatePost);
        router.add_regex(Method::Put, "^/api/posts/?$", Route::UpdatePost);
        router.add_regex(Method::Delete, "^/api/posts/(?P<id>[:digit:]+)/?$", Route::DeletePost);
        router
    }
}

impl HttpHandler for Route {
    type Context = BlogContext;

    fn exec(&self, ctx: Self::Context, req: Request, res: Response) {
        info!("Matched Route {:?}", self);
        let posts = PostService::new(ctx.redis.get().unwrap());
        match *self {
            Route::GetPosts => {
                posts.list(10, 0);
            },
            Route::GetPost => {
                posts.get(1);
            },
            _ => {}
        }
        res.send(b"Hello World").ok();
    }
}
