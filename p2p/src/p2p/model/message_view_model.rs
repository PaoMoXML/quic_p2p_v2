use std::sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicUsize},
};

use chrono::{DateTime, FixedOffset, Local, Utc};
use ratatui::widgets::ScrollbarState;
use serde::{Deserialize, Serialize};

use super::{AutoScrollableViewModel, ViewModelFmt};

static LOCAL_FIXED_OFFSET: LazyLock<FixedOffset> = LazyLock::new(|| {
    let l_now = Local::now();
    let l_offset_time = l_now.fixed_offset();
    *l_offset_time.offset()
});

#[derive(Clone, Debug, Default)]
pub struct MessageViewModel {
    myself_id: String,
    messages: Vec<Message>,
    vertical_scroll_state: Arc<Mutex<ScrollbarState>>,
    vertical_scroll: Arc<AtomicUsize>,
    vertical_scroll_at_end: Arc<AtomicBool>,
}

impl MessageViewModel {
    pub fn new(myself_id: String) -> Self {
        Self {
            myself_id,
            ..Default::default()
        }
    }

    pub fn get_myself_id(&self) -> &str {
        &self.myself_id
    }
    pub fn add_msg(&mut self, msg: Message) {
        self.messages.push(msg);
    }

    pub fn vertical_scroll_down(&mut self) {
        // let vertical_scroll = self
        //     .vertical_scroll
        //     .load(std::sync::atomic::Ordering::Relaxed);
        // 防止下拉太多
        // if vertical_scroll < self.messages.len().saturating_sub(1) {}
        self.vertical_scroll
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.position(
            self.vertical_scroll
                .load(std::sync::atomic::Ordering::Relaxed),
        );

        self.reset_vertical_scroll_at_end();
    }

    pub fn vertical_scroll_up(&mut self) {
        if self
            .vertical_scroll
            .load(std::sync::atomic::Ordering::Relaxed)
            <= 1
        {
            self.vertical_scroll
                .store(0, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.vertical_scroll
                .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }

        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.position(
            self.vertical_scroll
                .load(std::sync::atomic::Ordering::Relaxed),
        );

        self.reset_vertical_scroll_at_end();
    }

    pub fn set_vertical_scroll_content_len(&self, len: usize) {
        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.content_length(len);
    }

    pub fn get_vertical_scroll_state(&self) -> Arc<Mutex<ScrollbarState>> {
        self.vertical_scroll_state.clone()
    }
}

impl AutoScrollableViewModel for MessageViewModel {
    type Msg = Message;
    fn get_inner_msgs(&self) -> &Vec<Self::Msg> {
        &self.messages
    }

    fn get_vertical_scroll(&self) -> Arc<AtomicUsize> {
        self.vertical_scroll.clone()
    }

    fn vertical_scroll_to_end(&self) {
        self.vertical_scroll_at_end
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_vertical_scroll_at_end(&self) -> Arc<AtomicBool> {
        self.vertical_scroll_at_end.clone()
    }

    fn reset_vertical_scroll_at_end(&self) {
        self.vertical_scroll_at_end
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn vertical_scroll_to(&self, pos: usize) {
        self.vertical_scroll
            .store(pos, std::sync::atomic::Ordering::Relaxed);
        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.position(pos);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    time: DateTime<Utc>,
    content: String,
    user: UserType,
    relay_route: Option<Vec<String>>,
}

impl Message {
    pub fn new(
        local_time: DateTime<Local>,
        content: String,
        user: UserType,
        relay_route: Option<Vec<String>>,
    ) -> Self {
        Self {
            time: local_time.to_utc(),
            content,
            user,
            relay_route,
        }
    }

    pub fn get_relay_route(&self) -> &Option<Vec<String>> {
        &self.relay_route
    }

    pub fn set_relay_route(&mut self, relay_route: Option<Vec<String>>) {
        if let Some(relay_route) = relay_route
            && !relay_route.is_empty()
        {
            self.relay_route = Some(relay_route)
        }
    }

    pub fn get_user(&self) -> &UserType {
        &self.user
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }
}

impl ViewModelFmt for Message {
    fn formart(&self) -> (Vec<String>, Option<String>) {
        let relay_route_msg = self
            .relay_route
            .as_ref()
            .map(|relay_route| format!(" [{}]", relay_route.join(" -> ")));
        let mut contents = Vec::new();
        for (index, splited_content) in self.content.split('\n').enumerate() {
            if index == 0 {
                contents.push(format!(
                    "({}): {}",
                    self.time
                        .with_timezone(&*LOCAL_FIXED_OFFSET)
                        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                    splited_content
                ));
            } else {
                contents.push(splited_content.to_string());
            }
        }

        (contents, relay_route_msg)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UserType {
    User(String),
    System,
}

#[test]
fn xx() {
    let x: usize = 1;
    let a = x.saturating_add(1);
    println!("{a}");
    let a = x.saturating_sub(100);
    println!("{a}");

    let x = AtomicUsize::new(0);

    x.fetch_sub(2, std::sync::atomic::Ordering::Relaxed);

    println!("{}", x.load(std::sync::atomic::Ordering::Relaxed));
}
