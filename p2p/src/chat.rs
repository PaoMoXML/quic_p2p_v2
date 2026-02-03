use std::{fmt::Display, task::Poll};

use chrono::{DateTime, Local, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::p2p::node::{P2PNode, message::MessagePayload, node_id::LocalNodeId};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    time: DateTime<Utc>,
    content: String,
    user: UserType,
}

impl ChatMessage {
    pub fn new(local_time: DateTime<Local>, content: String, user: UserType) -> Self {
        Self {
            time: local_time.to_utc(),
            content,
            user,
        }
    }

    pub fn get_user(&self) -> &UserType {
        &self.user
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }
}

impl Display for ChatMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] => {}",
            self.user,
            self.time.to_rfc3339(),
            self.content
        )
    }
}

impl MessagePayload for ChatMessage {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum UserType {
    User(LocalNodeId),
    System,
}

impl Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserType::User(local_node_id) => write!(f, "{}", local_node_id.short()),
            UserType::System => write!(f, "System"),
        }
    }
}

#[derive(Debug)]
pub struct ChatNode {
    inner: P2PNode<ChatMessage>,
    message_rx: mpsc::Receiver<ChatMessage>,
    cloned_token: CancellationToken,
}

impl ChatNode {
    pub fn new(
        node: P2PNode<ChatMessage>,
        message_rx: mpsc::Receiver<ChatMessage>,
        cloned_token: CancellationToken,
    ) -> Self {
        Self {
            inner: node,
            message_rx,
            cloned_token,
        }
    }
}

impl Future for ChatNode {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let chat_node = self.get_mut();

        // 处理来自内部P2P节点的消息
        while let Poll::Ready(Some(m)) = chat_node.inner.poll_next_unpin(cx) {
            println!("# MESSAGE: {}", m.payload);
            // 退出
            if m.payload.get_content() == "quit" {
                chat_node.inner.leave();
                chat_node.cloned_token.cancel();
            }
        }

        // 处理来自消息接收器的消息
        while let Poll::Ready(Some(msg)) = chat_node.message_rx.poll_recv(cx) {
            chat_node.inner.broadcast(msg);
        }

        Poll::Pending
    }
}
