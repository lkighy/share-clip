import type React from 'react';
import { Code, FileText, Folder, Image as ImageIcon, File } from 'lucide-react';

import { ClipboardType } from '@/models/clipboardRecord.ts';

export interface ClipboardResponse {
  id: number;
  type: ClipboardType;
  preview?: string;
  hash?: string;
  size?: number;
  source_app?: string;
  created_at: number;
  last_accessed_at: number;
  access_count: number;
  is_favorite: number;
  is_shared: number;
  is_valid: number,
}

export const TypeIconMap: Record<ClipboardType, React.ElementType> = {
  [ClipboardType.Text]: FileText,
  [ClipboardType.Html]: Code,
  [ClipboardType.Rtf]: Code,
  [ClipboardType.Image]: ImageIcon,
  [ClipboardType.File]: File,
  [ClipboardType.Folder]: Folder,
};

export const TypeNameMap: Record<ClipboardType, string> = {
  [ClipboardType.Text]: '文本',
  [ClipboardType.Rtf]: '富文本',
  [ClipboardType.Html]: 'HTML',
  [ClipboardType.Image]: '图片',
  [ClipboardType.File]: '文件',
  [ClipboardType.Folder]: '文件夹',
};
