
//
import {ClipboardResponse} from "@/api/types/clipboardRecord.ts";
import {call} from "./core.ts";

// 获取剪切板列表
export function clipboardRecordList(page: number, pageSize: number) {
    return call<ClipboardResponse[]>("clipboard_record_list", {page, pageSize})
}

// 添加粘贴事件
export function setClipboardContent(id: number) {
    return call("paste_clipboard_record", {id})
}

// 复制事件
export function copyClipboardContent(id: number) {
    return call("copy_clipboard_record", {id})
}

// 收藏
export function toggleFavorite(id: number) {
    return call<boolean>("toggle_favorite", {id})
}

// 分享
export function toggleShare(id: number) {
    return call<boolean>("toggle_share", {id})
}

// 删除
export function deleteClipboardRecord(id: number) {
    return call("delete_clipboard_record", {id})
}
