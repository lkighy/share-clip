
export interface ClipboardResponseModel {
    id: number
    type: ClipboardType
    preview?: string
    hash?: string
    size?: number
    sourceApp?: string
    createdAt: Date
    lastAccessedAt?: Date
    accessCount: number
    isFavorite: boolean
    isShared: boolean
    isValid: boolean
}

export enum ClipboardType {
    Text = 0,
    Html = 1,
    Rtf = 2,
    Image = 3,
    File = 4,
    Folder = 5,
}