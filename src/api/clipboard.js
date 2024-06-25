// 剪切板相关API

import {invoke} from "@tauri-apps/api/core";

async function get_clip_text() {
    return await invoke("get_clipboard_text")
}