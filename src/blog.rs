use posts::{Post, PostService};
use web_server::{RouteHandler, Router, RouteParams, Request, Response};

use regex::Regex;

use r2d2::Pool;
use r2d2_redis::RedisConnectionManager;

pub struct BlogContext {
    posts: PostService,
    redis: Pool<RedisConnectionManager>,
}

impl BlogContext {
    pub fn new(redis: Pool<RedisConnectionManager>) -> BlogContext {
        BlogContext {
            posts: PostService::new(redis.clone()),
            redis: redis
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
        router.get(Regex::new("^/api/posts/?$").unwrap(), Route::GetPosts);
        router.post(Regex::new("^/api/posts/?$").unwrap(), Route::CreatePost);
        router.put(Regex::new("^/api/posts/?$").unwrap(), Route::UpdatePost);
        router.delete(Regex::new("^/api/posts/(?P<id>[:digit:]+)/?$").unwrap(), Route::DeletePost);
        router
    }
}

impl RouteHandler for Route {
    type Context = BlogContext;

    fn route<'a, 'b: 'a, 'c>(&self, ctx: &mut BlogContext, req: Request<'a, 'b>, res: Response<'c>, params: RouteParams)
             -> (Request<'a, 'b>, Response<'c>) {
        info!("Matched Route {:?}", self);

        match *self {
            Route::GetPosts => {
                let limit = req.query_param("limit")
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(10);
                let skip = req.query_param("skip")
                    .and_then(|x| x.parse().ok())
                    .unwrap_or(0);

                let posts = ctx.posts.list(limit, skip);

                (req, res.json(&posts))
            },
            Route::GetPost => {
                let post = ctx.posts.get(params.get("id").unwrap().parse().unwrap());

                (req, res.json(&post))
            },
            Route::GetPostByFragment => {
                let post = ctx.posts.get_by_fragment(params.get("fragment").unwrap());

                (req, res.json(&post))
            },
            Route::DeletePost => {
                ctx.posts.delete(params.get("id").unwrap().parse().unwrap());
                (req, res)
            },
            Route::UpdatePost => {
                let mut req = req.body();
                let post = req.to_json().unwrap();

                ctx.posts.update(post);
                (req, res)
            },
            Route::CreatePost => {
                let mut req = req.body();
                let post = req.to_json().unwrap();

                let post = ctx.posts.create(post);
                (req, res)
            },
        }
    }
}
