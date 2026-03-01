import {ClipboardResponseModel, ClipboardType} from "@/models/clipboardRecord.ts";
import {ClipboardResponse} from "@/api/types/clipboardRecord.ts";


export function mapClipboardRecord(dto: ClipboardResponse): ClipboardResponseModel {
    return {
        id: dto.id,
        type: dto.type as ClipboardType,
        preview: dto.preview ?? undefined,
        hash: dto.hash ?? undefined,
        size: dto.size ?? undefined,
        sourceApp: dto.source_app ?? undefined,
        createdAt: new Date(dto.created_at * 1000),
        lastAccessedAt: dto.last_accessed_at
            ? new Date(dto.last_accessed_at * 1000)
            : undefined,
        accessCount: dto.access_count,
        isFavorite: dto.is_favorite === 1,
        isShared: dto.is_shared === 1
    }
}