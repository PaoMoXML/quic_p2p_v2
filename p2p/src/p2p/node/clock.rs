use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::p2p::node::node_id::NodeId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorClock {
    node_id: NodeId,
    timestamps: HashMap<NodeId, u64>, // 每个节点的逻辑时钟
}

impl VectorClock {
    pub fn new(node_id: NodeId) -> Self {
        let mut timestamps = HashMap::new();
        timestamps.insert(node_id.clone(), 0);
        VectorClock {
            node_id,
            timestamps,
        }
    }

    // 更新来自特定节点的时间戳
    pub fn update_from(&mut self, sender: &NodeId, sender_timestamp: u64) {
        let current = self.timestamps.entry(sender.clone()).or_insert(0);
        *current = (*current).max(sender_timestamp);
    }

    // 递增自己的时钟
    pub fn increment(&mut self) {
        let current = self.timestamps.entry(self.node_id.clone()).or_insert(0);
        *current += 1;
    }

    // 获取自己的时钟值
    pub fn get_own_timestamp(&self) -> u64 {
        *self.timestamps.get(&self.node_id).unwrap_or(&0)
    }

    // 比较两个向量时钟的关系
    pub fn compare_with(&self, other: &VectorClock) -> VectorClockOrdering {
        let self_is_greater = self.timestamps.iter().all(|(node, ts)| {
            other
                .timestamps
                .get(node)
                .is_some_and(|other_ts| ts >= other_ts)
        }) && self.timestamps.iter().any(|(node, ts)| {
            other
                .timestamps
                .get(node)
                .is_some_and(|other_ts| ts > other_ts)
        });

        let other_is_greater = other.timestamps.iter().all(|(node, ts)| {
            self.timestamps
                .get(node)
                .is_some_and(|self_ts| ts < self_ts)
        }) && other.timestamps.iter().any(|(node, ts)| {
            self.timestamps
                .get(node)
                .is_some_and(|self_ts| ts < self_ts)
        });

        if self_is_greater && other_is_greater {
            VectorClockOrdering::Concurrent
        } else if self_is_greater {
            VectorClockOrdering::After
        } else if other_is_greater {
            VectorClockOrdering::Before
        } else {
            VectorClockOrdering::Equal
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum VectorClockOrdering {
    Before,     // self happens before other
    After,      // self happens after other
    Concurrent, // events are concurrent (not causally related)
    Equal,      // clocks are equal
}
