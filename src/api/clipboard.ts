
//
import {ClipboardResponse} from "@/api/types/clipboardRecord.ts";
import {call} from "./core.ts";

// 获取剪切板列表
export function clipboardRecordList(page: number, pageSize: number) {
    return call<ClipboardResponse[]>("clipboard_record_list", {page, pageSize})
}

// TODO: 添加粘贴事件
export function setClipboardContent(id: number) {
    return call("paste", {id})
}