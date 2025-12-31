use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
struct GptMessageToSend {
    role: String,
    content: String,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct GptMessageToSends {
    messages: Vec<GptMessageToSend>,
}

impl GptMessageToSends {
    pub fn add_user_msg(&mut self, content: String) {
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
        self.messages.push(GptMessageToSend {
            role: "user".to_string(),
            content,
        });
    }

    pub fn add_assistant_msg(&mut self, content: String) {
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
        self.messages.push(GptMessageToSend {
            role: "assistant".to_string(),
            content,
        });
    }

    pub fn add_system_msg(&mut self, content: String) {
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
        self.messages.push(GptMessageToSend {
            role: "system".to_string(),
            content,
        });
    }

    pub fn to_json_str(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}
