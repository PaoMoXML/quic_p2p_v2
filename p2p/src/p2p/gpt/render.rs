use std::{
    io,
    sync::{LazyLock, Mutex},
};

use markdown::ParseOptions;
use serde_json::Value;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

static LIST_STACK: LazyLock<Mutex<Vec<usize>>> = LazyLock::new(|| Mutex::new(Vec::new()));

static CONTENT_INDENT_LEVEL: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

static ORDERED_LIST_STACK: LazyLock<Mutex<Vec<bool>>> = LazyLock::new(|| Mutex::new(Vec::new()));

#[test]
fn markdown_test() {
    let text = r#"
## 黄金价格上涨对银行股的影响比较复杂，不能简单判断为"带动上涨"，需要分情况来看：

1. **利空因素**：
    - 黄金大涨往往伴随市场避险情绪升温，可能反映经济前景不佳，这对银行资产质量是负面信号
    - 央行在黄金大涨时可能维持紧缩政策，抬高银行资金成本
    - 投资者资金可能从股市(包括银行股)流向黄金

2. **利好因素**：
    - 部分银行持有黄金头寸或开展黄金业务，可能直接受益
    - 通胀环境下黄金上涨时，银行净息差可能扩大（但高通胀对银行整体弊大于利）
    - 黄金相关金融产品（如纸黄金、黄金ETF托管等）交易活跃度提升

3. **实际情况**：
    - 国内银行股主要受宏观经济、房地产政策、息差变化等影响
    - 黄金价格单一因素对银行板块影响权重通常不超过5%
    - 2020年黄金历史性大涨期间，银行股整体表现平平

建议投资者：
- 关注银行基本面而非黄金价格
- 如果特别看好黄金，可直接配置黄金ETF或矿业股
- 银行股更适合作为宏观经济回暖时的配置选择

（数据提示：近10年黄金与银行股相关性仅0.2左右，属弱相关）
[ finish: stop, total token: 293 ]
    "#;

    let text = r#"## Hello world"#;

    // println!("text: {text}");

    let ast = markdown::to_mdast(text, &ParseOptions::default());
    // println!("ast: {ast:?}");

    let ast = markdown::to_mdast(text, &ParseOptions::gfm()).unwrap();
    // println!("ast: {ast:?}");

    let json: Value = serde_json::from_str(&serde_json::to_string(&ast).unwrap())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))
        .unwrap();

    println!("{json}");
    println!();
    render_node(&json).unwrap();
}

pub fn render_node(node: &Value) -> io::Result<()> {
    match node["type"].as_str() {
        Some("root") => render_children(node)?,
        Some("heading") => render_heading(node)?,
        Some("paragraph") => render_paragraph(node)?,
        Some("text") => render_text(node)?,
        // Some("table") => render_table(node)?,
        Some("list") => render_list(node)?,
        Some("listItem") => render_list_item(node)?,
        // Some("blockquote") => render_blockquote(node)?,
        // Some("thematicBreak") => render_thematic_break()?,
        // Some("emphasis") => render_emphasis(node)?,
        Some("strong") => render_strong(node)?,
        _ => {
            // render_children(node).unwrap();
            // println!("?{}?", );
            println!("Unsupported node type: {:?}", node["type"]);
        }
    }
    Ok(())
}

fn render_children(node: &Value) -> io::Result<()> {
    if let Some(children) = node["children"].as_array() {
        for child in children {
            render_node(child)?;
        }
    }
    Ok(())
}

fn render_heading(node: &Value) -> io::Result<()> {
    let _depth = node["depth"].as_u64().unwrap_or(1);

    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(
        ColorSpec::new()
            .set_fg(Some(Color::Cyan))
            .set_italic(true)
            .set_bold(true),
    )?;
    render_children(node)?;
    stdout.reset()?;

    println!();

    Ok(())
}

fn render_text(node: &Value) -> io::Result<()> {
    let text = node["value"].as_str().unwrap_or("");
    let words: Vec<&str> = text.split_whitespace().collect();

    for (i, word) in words.iter().enumerate() {
        if i > 0 {
            print!(" ");
        }
        print!("{}", word);
    }
    Ok(())
}

fn render_paragraph(node: &Value) -> io::Result<()> {
    // Only print indent if it's not inside a list item
    if let Ok(list_stack) = LIST_STACK.lock()
        && list_stack.is_empty()
    {
        print!("{}", get_indent());
    }
    render_children(node)?;
    println!();
    Ok(())
}

fn render_list(node: &Value) -> io::Result<()> {
    let is_ordered = node["ordered"].as_bool().unwrap_or(false);
    if let Ok(mut list_stack) = LIST_STACK.lock() {
        list_stack.push(0);
    }
    if let Ok(mut ordered_list_stack) = ORDERED_LIST_STACK.lock() {
        ordered_list_stack.push(is_ordered);
    }
    render_children(node)?;
    if let Ok(mut list_stack) = LIST_STACK.lock() {
        list_stack.pop();
    }
    if let Ok(mut ordered_list_stack) = ORDERED_LIST_STACK.lock() {
        ordered_list_stack.pop();
    }
    Ok(())
}

fn render_list_item(node: &Value) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    print!("{}", get_indent());

    {
        let mut list_stack = LIST_STACK.lock().unwrap();
        let ordered_list_stack = ORDERED_LIST_STACK.lock().unwrap();

        if let Some(index) = list_stack.last_mut() {
            *index += 1;
            if *ordered_list_stack.last().unwrap_or(&false) {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))?;
                print!("{:2}. ", *index);
            } else {
                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                print!("• ");
            }
        } else {
            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
            print!("• ");
        }
    }
    stdout.reset()?;

    if let Some(checked) = node["checked"].as_bool() {
        render_task_list_item_checkbox(checked)?;
    }

    if let Ok(mut content_indent_level) = CONTENT_INDENT_LEVEL.lock() {
        *content_indent_level += 1;
    }

    render_children(node)?;

    if let Ok(mut content_indent_level) = CONTENT_INDENT_LEVEL.lock() {
        *content_indent_level -= 1;
    }

    // println!(); // Add a newline after each list item
    Ok(())
}

fn render_task_list_item_checkbox(checked: bool) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);

    if checked {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        print!("  ");
    } else {
        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))?;
        print!("  ");
    }
    stdout.reset()?;
    Ok(())
}

fn render_strong(node: &Value) -> io::Result<()> {
    let mut stdout = StandardStream::stdout(ColorChoice::Always);
    stdout.set_color(ColorSpec::new().set_bold(true))?;
    render_children(node)?;
    stdout.reset()?;
    Ok(())
}

pub fn get_indent() -> String {
    if let Ok(content_indent_level) = CONTENT_INDENT_LEVEL.lock() {
        "  ".repeat(*content_indent_level)
    } else {
        String::new() // Return empty string if lock fails
    }
}
