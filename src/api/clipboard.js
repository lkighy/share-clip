// 剪切板相关API

import {invoke} from "@tauri-apps/api/core";

export async function getClipText() {
    return await invoke("get_clipboard_text")
}