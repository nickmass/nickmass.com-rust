use r2d2;
use r2d2_redis::RedisConnectionManager;
use redis;
use redis::Commands;

use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug)]
pub struct Post {
    pub id: Option<u32>,
    pub title: String,
    pub content: String,
    pub date: u64,
    pub author_id: u32,
    pub url_fragment: String,
}

impl redis::FromRedisValue for Post {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Post> {
        let mut post_fields: HashMap<String, redis::Value> = 
            try!(HashMap::from_redis_value(v));

        let id = redis::from_redis_value(&post_fields.remove("id").unwrap());
        let title = redis::from_redis_value(&post_fields.remove("title").unwrap());
        let content = redis::from_redis_value(&post_fields.remove("content").unwrap());
        let date = redis::from_redis_value(&post_fields.remove("date").unwrap());
        let author_id = redis::from_redis_value(&post_fields.remove("authorId").unwrap());
        let url_frag = redis::from_redis_value(&post_fields.remove("urlFragment").unwrap());

        Ok(Post {
            id: Some(try!(id)),
            title: try!(title),
            content: try!(content),
            date: try!(date),
            author_id: try!(author_id),
            url_fragment: try!(url_frag),
        })
    }
}

pub struct PostService {
    db: r2d2::Pool<RedisConnectionManager>
}

impl PostService {
    pub fn new(redis: r2d2::Pool<RedisConnectionManager>) -> PostService {
        PostService{
            db: redis
        }
    }

    pub fn list(&self, limit: u32, skip: u32) -> Vec<Post> {
        let db = self.db.get().unwrap();
        Vec::new()
    }

    pub fn get(&self, id: u32) -> Post {
        let db = self.db.get().unwrap();
        db.hgetall(&format!("posts:{}", id)).unwrap()
    }

    pub fn get_by_fragment(&self, frag: &str) -> Post {
        let db = self.db.get().unwrap();
        let post_id: u32 = 
            db.get(&format!("postFragment:{}", frag)).unwrap();
        self.get(post_id)
    }

    pub fn create(&self, post: Post) -> Post {
        let db = self.db.get().unwrap();
        post
    }

    pub fn update(&self, post: Post) -> Post {
        if post.id.is_none() { return post; }

        let db = self.db.get().unwrap();
        let exists: bool = db.get(&format!("posts:{}", post.id.unwrap())).unwrap();
        post
    }

    pub fn delete(&self, id: u32) { 
        let db = self.db.get().unwrap();
    }
}
