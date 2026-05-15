
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

export type RemoteClipboardContent = {
    id: number;
    type: number;
    text?: string | null;
    html?: string | null;
    rtf?: string | null;
    image_base64?: string | null;
    files?: string[] | null;
    sync_targets?: RemoteClipboardSyncTarget[] | null;
}

export type RemoteClipboardSyncTarget = {
    share_id: string;
    share_name: string;
    relative_path: string;
    name: string;
    is_dir: boolean;
    size?: number | null;
    mtime?: number | null;
    hash?: string | null;
}

export type RemoteClipboardContentPayload = Omit<RemoteClipboardContent, "id" | "sync_targets">;

export function copyRemoteClipboardContent(payload: RemoteClipboardContentPayload) {
    return call<void>("copy_remote_clipboard_content", {payload})
}

export function pasteRemoteClipboardContent(payload: RemoteClipboardContentPayload) {
    return call<void>("paste_remote_clipboard_content", {payload})
}
