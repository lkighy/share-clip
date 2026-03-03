import {
    clipboardRecordList,
    copyClipboardContent,
    deleteClipboardRecord,
    setClipboardContent,
    toggleFavorite,
    toggleShare
} from "@/api/clipboard.ts";
import {mapClipboardRecord} from "@/models/clipboardRecord.mapper.ts";

export async function getClipboardRecordList(page: number, pageSize: number) {
    const res = await clipboardRecordList(page, pageSize);

    return res.map(mapClipboardRecord);
}

/// 粘贴内容
export async function pasteItem(id: number) {
    await setClipboardContent(id)
}

export async function copyItem(id: number) {
    await copyClipboardContent(id)
}

export async function handleFavoriteToggle(id: number) {
    return await toggleFavorite(id)
}

export async function handleShareToggle(id: number) {
    return await toggleShare(id)
}

export async function removeItem(id: number) {
    await deleteClipboardRecord(id)
}
