// 窗口相关操纵
#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowOperations {
    // 关闭
    Close,
    // 隐藏
    Hide,
}

// 窗口label
pub enum WindowLabel {
    #[allow(dead_code)]
    Clipboard, // 剪切板
    Config,    // 设置
    ShareFile, // 分享文件
}

impl WindowLabel {
    pub fn label(&self) -> &'static str {
        match self {
            WindowLabel::Clipboard => "index",
            WindowLabel::Config => "app-config",
            WindowLabel::ShareFile => "shared-files",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            WindowLabel::Clipboard => "剪切板",
            WindowLabel::Config => "设置",
            WindowLabel::ShareFile => "文件",
        }
    }

    pub fn url_params(&self) -> &'static str {
        match self {
            WindowLabel::Clipboard => "",
            WindowLabel::Config => "app-config",
            WindowLabel::ShareFile => "shared-files",
        }
    }
}
