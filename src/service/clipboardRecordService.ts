import {clipboardRecordList, setClipboardContent} from "@/api/clipboard.ts";
import {mapClipboardRecord} from "@/models/clipboardRecord.mapper.ts";

export async function getClipboardRecordList(page: number, pageSize: number) {
    const res = await clipboardRecordList(page, pageSize);

    return res.map(mapClipboardRecord);
}

/// 粘贴内容
export async function pasteItem(id: number) {
    await setClipboardContent(id)
}