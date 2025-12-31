use std::{collections::HashMap, fs, io::Read, pin::Pin, sync::LazyLock};

use futures::{Stream, StreamExt};
use rootcause::Report;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use super::{Chat, common::GptMessageToSends};

/// client缓存
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);
/// 聊天缓存
static CHAT_CACHE: LazyLock<Mutex<HashMap<String, GptMessageToSends>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct DeepSeekV3 {
    token: String,
    url: String,
    model: String,
}

impl DeepSeekV3 {
    pub fn init_from_toml() -> Result<Self, Report> {
        let mut file = fs::File::open("./deepseek.toml")?;
        let mut str = String::new();
        file.read_to_string(&mut str)?;
        Ok(toml::from_str(&str)?)
    }

    pub async fn chat_stream(
        &self,
        input: String,
        user: String,
    ) -> Result<Pin<Box<dyn Stream<Item = String> + Send + Sync + '_>>, Report> {
        let post = HTTP_CLIENT.post(self.url.clone());
        // 获取聊天缓存，如果没有则初始化
        let mut chat_cache = CHAT_CACHE.lock().await;
        let messages = match chat_cache.get_mut(&user) {
            Some(msgs) => msgs,
            None => {
                chat_cache.insert(user.clone(), GptMessageToSends::default());
                let msgs = chat_cache.get_mut(&user).unwrap();
                msgs.add_system_msg("你个一个聊天机器人，不使用markdown格式".to_string());
                msgs
            }
        };
        // 添加用户聊天内容
        messages.add_user_msg(input);
        // 组装request body
        let mut msg_json = messages.to_json();
        msg_json["model"] = Value::String(self.model.to_string());
        msg_json["stream"] = Value::Bool(true);
        let result_stream = post
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.token))
            .body(msg_json.to_string())
            .send()
            .await?
            .bytes_stream();

        // 将 BytesStream 转换成 String Stream
        let stream = result_stream
            .filter_map(move |chunk| async move {
                let chunk = chunk.ok()?;
                let word = String::from_utf8_lossy(&chunk);
                let mut results = Vec::new();

                for line in word.lines() {
                    if let Some(json_str) = line.strip_prefix("data: ") {
                        if json_str.is_empty() || json_str == "[DONE]" {
                            continue;
                        }
                        if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                            // println!("json: {json}");
                            if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                                results.push(content.to_string());
                            }
                            if !json["choices"][0]["finish_reason"].is_null() {
                                results.push(String::from("\n"));
                                results.push(format!(
                                    "[ finish: {}, total token: {} ]",
                                    json["choices"][0]["finish_reason"].as_str().unwrap(),
                                    json["usage"]["total_tokens"].as_u64().unwrap(),
                                ));
                            }
                        }
                    }
                }

                if results.is_empty() {
                    None
                } else {
                    Some(futures::stream::iter(results))
                }
            })
            .flatten(); // 将 Vec<String> 转换为多个 String 项

        Ok(Box::pin(stream))
    }
}

impl Chat for DeepSeekV3 {
    async fn chat(&self, input: String, user: String) -> Result<String, Report> {
        let post = HTTP_CLIENT.post(self.url.clone());
        // 获取聊天缓存，如果没有则初始化
        let mut chat_cache = CHAT_CACHE.lock().await;
        let messages = match chat_cache.get_mut(&user) {
            Some(msgs) => msgs,
            None => {
                chat_cache.insert(user.clone(), GptMessageToSends::default());
                let msgs = chat_cache.get_mut(&user).unwrap();
                msgs.add_system_msg("你个一个聊天机器人，不使用markdown格式".to_string());
                msgs
            }
        };
        // 添加用户聊天内容
        messages.add_user_msg(input);
        // 组装request body
        let mut msg_json = messages.to_json();
        msg_json["model"] = Value::String(self.model.to_string());
        let result_json: Value = post
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .header("Authorization", format!("Bearer {}", self.token))
            .body(msg_json.to_string())
            .send()
            .await?
            .json()
            .await?;
        let content = &result_json["choices"][0]["message"]["content"];
        // 将ai的回复添加进聊天缓存
        let content_str = content.as_str().unwrap();
        messages.add_assistant_msg(content_str.to_string());
        Ok(content_str.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use futures::StreamExt;
    use markdown::ParseOptions;
    use serde_json::Value;

    use crate::p2p::gpt::{Chat, deepseek_v3::DeepSeekV3, render::render_node};

    #[tokio::test]
    async fn test_deepseek() {
        let d = DeepSeekV3::init_from_toml().unwrap();
        // let x = d.chat("你好，你叫什么".to_string(), "xml".to_string()).await.unwrap();
        // println!("{x}");
        // let x = d.chat("我叫xml".to_string(), "xml".to_string()).await.unwrap();
        // println!("{x}");
        // let x = d.chat("我叫什么".to_string(), "xml".to_string()).await.unwrap();
        // println!("{x}");
        // let x = d.chat("我叫什么".to_string(), "xml2".to_string()).await.unwrap();
        // println!("{x}");
        let x = d
            .chat("今年的诺贝尔奖都颁给了谁".to_string(), "xml2".to_string())
            .await
            .unwrap();
        println!("{x}");
        // let xx = "asd\n\nasdasd".to_string();

        // println!("{xx}");
        // let xxxx = "2023年诺贝尔奖的获奖名单如下，每个奖项的得主及其主要贡献如下：\n\n### **诺贝尔生理学或医学奖**\n- **卡塔琳·卡里科（Katalin Karikó）** 和 **德鲁·魏斯曼（Drew Weissman）**\n  - **贡献**：他们在mRNA技术上的突破性研究，为新冠疫苗的快速开发奠定了基础。\n\n### **诺贝尔物理学奖**\n- **皮埃尔·阿戈斯蒂尼（Pierre Agostini）**、**费伦茨·克劳斯（Ferenc Krausz）** 和 **安妮·卢利耶（Anne L’Huillier）**\n  - **贡献**：他们在超快激光科学和阿秒物理领域的实验性贡献，使研究电子超快运动成为可能。\n\n### **诺贝尔化学奖**\n- **蒙吉·巴文迪（Moungi G. Bawendi）**、**路易斯·布鲁斯（Louis E. Brus）** 和 **阿列克谢·叶基莫夫（Alexei I. Ekimov）**\n  - **贡献**：他们在量子点的发现与合成方面的开创性工作，推动了纳米技术的发展。\n\n### **诺贝尔文学奖**\n- **约恩·福瑟（Jon Fosse）**（挪威作家）\n  - **贡献**：他的戏剧和散文作品以简约而深刻的方式探索了人类存在的本质。\n\n### **诺贝尔和平奖**\n- **纳尔吉斯·穆罕默迪（Narges Mohammadi）**（伊朗人权活动家）\n  - **贡献**：她为伊朗妇女权利和民主自由进行的长期抗争，尽管面临监禁仍坚持发声。\n\n### **诺贝尔经济学奖**（正式名称为“瑞典中央银行纪念阿尔弗雷德·诺贝尔经济学奖”）\n- **克劳迪娅·戈尔丁（Claudia Goldin）**\n  - **贡献**：她对女性劳动力市场历史的研究，揭示了性别收入差距的深层原因。\n\n如果你对某个奖项或获奖者想了解更多细节，欢迎继续提问！";
        // println!("{xxxx}");
    }

    #[tokio::test]
    async fn test_deepseek_stream() {
        let d = DeepSeekV3::init_from_toml().unwrap();
        let mut stream = d
            .chat_stream(
                "黄金价格上涨会带动银行股上涨吗".to_string(),
                "xml2".to_string(),
            )
            .await
            .unwrap();
        let mut text = String::new();
        while let Some(content) = stream.next().await {
            print!("{}", content);
            std::io::stdout().flush().unwrap();
            text.push_str(&content);
        }
        println!();

        let ast = markdown::to_mdast(&text, &ParseOptions::gfm()).unwrap();
        let json: Value = serde_json::from_str(&serde_json::to_string(&ast).unwrap())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
            .unwrap();
        render_node(&json).unwrap();
    }
}
