use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};

use super::model::message_view_model::Message;

struct GossipSub {
    // 网络期望的出度
    d: usize,
    // 出度的下限
    d_low: usize,
    // 出度上限
    d_high: usize,
    // （可选）流言传播的外向程度
    d_lazy: Option<usize>,
    // 心跳间隔时间
    heartbeat_interval: Duration,
    // 每个主题的扇出状态的生存时间
    fanout_ttl: Duration,
    // 消息缓存中的历史窗口数量
    mcache_len: usize,
    // 在传播消息时使用的“历史窗口”数量
    mcache_gossip: usize,
    // 已读消息 ID 缓存的过期时间
    seen_ttl: Duration,
}

impl Default for GossipSub {
    fn default() -> Self {
        Self {
            d: 6,
            d_low: 4,
            d_high: 12,
            d_lazy: Some(6),
            heartbeat_interval: Duration::from_secs(1),
            fanout_ttl: Duration::from_secs(60),
            mcache_len: 6,
            mcache_gossip: 3,
            seen_ttl: Duration::from_secs(120),
        }
    }
}

struct PeeringState {
    peers: Vec<String>,
    mesh: HashMap<String, Vec<String>>,
    fanout: HashMap<String, Vec<String>>,
}

struct MessageCache {
    mcache_len: usize,
    windows: Vec<MessageCacheInner>,
}

impl MessageCache {
    fn put(&mut self, m: MessageCacheInner) {
        self.windows.push(m);
    }

    fn get(&self, id: &str) -> Option<&MessageCacheInner> {
        self.windows.iter().find(|&inner| inner.id == id)
    }

    fn get_gossip_ids(&self, topic: &str) -> Option<Vec<&MessageCacheInner>> {
        let mut res = Vec::new();
        for inner in &self.windows {
            if inner.topic == topic {
                res.push(inner);
            }
        }
        if res.is_empty() { None } else { Some(res) }
    }

    fn shift(&mut self) {
        let res = self
            .windows
            .drain(self.windows.len().saturating_sub(self.mcache_len)..)
            .collect::<Vec<MessageCacheInner>>();
        self.windows = res;
    }
}

struct MessageCacheInner {
    id: String,
    topic: String,
    cache_time: DateTime<Utc>,
    msg: Message,
}

#[test]
fn t() {
    let mcache_len = 13;
    let mut windows = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let x = windows
        .drain(windows.len().saturating_sub(mcache_len)..)
        .collect::<Vec<i32>>();
    windows = x;
    dbg!(windows);
}
