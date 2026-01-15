use redis::{aio::MultiplexedConnection, AsyncCommands, Client, Script};
use std::sync::Arc;
use tokio::sync:: Mutex;

#[derive(Clone)]
pub struct RateLimiter {
    conn: Arc<Mutex<MultiplexedConnection>>,
    script: Arc<Script>,
}

#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed:  bool,
    pub remaining: i64,
    pub retry_after_ms:  i64,
}

impl RateLimiter {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = Client::open(redis_url)?;
        let conn = client.get_multiplexed_tokio_connection().await?;

        let script = Script::new(LUA_SCRIPT);

        Ok(Self {
            conn:  Arc::new(Mutex::new(conn)),
            script:  Arc::new(script),
        })
    }

    pub async fn check(
        &self,
        client_id: &str,
        requests_per_minute:  i64,
        burst: i64,
    ) -> Result<RateLimitResult, redis::RedisError> {
        let key = format!("ratelimit:{}", client_id);
        let max_tokens = burst.max(1);
        let refill_rate = requests_per_minute as f64 / 60.0;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std:: time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let mut conn = self. conn.lock().await;

        let result:  Vec<i64> = self
            .script
            .key(&key)
            .arg(max_tokens)
            .arg(refill_rate)
            .arg(now_ms)
            .invoke_async(&mut *conn)
            .await?;

        Ok(RateLimitResult {
            allowed: result[0] == 1,
            remaining:  result[1],
            retry_after_ms: result[2],
        })
    }

    pub async fn ping(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.conn.lock().await;
        redis::cmd("PING").query_async(&mut *conn).await
    }
}

const LUA_SCRIPT: &str = r#"
local key = KEYS[1]
local max_tokens = tonumber(ARGV[1])
local refill_rate = tonumber(ARGV[2])
local now_ms = tonumber(ARGV[3])

local data = redis.call('HMGET', key, 'tokens', 'last_refill')
local tokens = tonumber(data[1])
local last_refill = tonumber(data[2])

if tokens == nil then
    tokens = max_tokens
    last_refill = now_ms
end

local elapsed_ms = now_ms - last_refill
local refill_amount = (elapsed_ms / 1000.0) * refill_rate
tokens = math.min(max_tokens, tokens + refill_amount)
last_refill = now_ms

local allowed = 0
local retry_after_ms = 0

if tokens >= 1 then
    tokens = tokens - 1
    allowed = 1
else
    local tokens_needed = 1 - tokens
    retry_after_ms = math.ceil((tokens_needed / refill_rate) * 1000)
end

redis.call('HMSET', key, 'tokens', tokens, 'last_refill', last_refill)
redis.call('EXPIRE', key, 120)

return {allowed, math.floor(tokens), retry_after_ms}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_logic() {
        let max_tokens:  f64 = 10.0;
        let refill_rate: f64 = 1.0; // 1 token per second (60 per minute)
        
        let mut tokens = max_tokens;
        let mut last_refill_ms:  i64 = 0;
        
        let consume = |tokens: &mut f64, last_refill: &mut i64, now_ms: i64| -> bool {
            let elapsed_ms = now_ms - *last_refill;
            let refill_amount = (elapsed_ms as f64 / 1000.0) * refill_rate;
            *tokens = f64::min(max_tokens, *tokens + refill_amount);
            *last_refill = now_ms;
            
            if *tokens >= 1.0 {
                *tokens -= 1.0;
                true
            } else {
                false
            }
        };

        for i in 0.. 10 {
            assert!(consume(&mut tokens, &mut last_refill_ms, 0), "Request {} should be allowed", i);
        }
        assert_eq!(tokens as i64, 0);

        assert! (!consume(&mut tokens, &mut last_refill_ms, 0), "11th request should be blocked");

        assert!(consume(&mut tokens, &mut last_refill_ms, 2000), "Should allow after 2 seconds");
    }

    #[test]
    fn test_retry_after_calculation() {
        let tokens:  f64 = 0.0;
        let refill_rate: f64 = 1.0;
        
        let tokens_needed = 1.0 - tokens;
        let retry_after_ms = ((tokens_needed / refill_rate) * 1000.0).ceil() as i64;
        
        assert_eq!(retry_after_ms, 1000);
    }
}