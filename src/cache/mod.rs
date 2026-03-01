// Cache module - 缓存模块
//
// 提供 Redis 缓存接口和实现

pub mod config;
pub mod key;
pub mod simple;

pub use config::CacheConfig;
pub use key::generate_cache_key;
pub use simple::{CachedResponse, SimpleCache};
