import React from 'react';
import { formatDistanceToNow } from 'date-fns';
import { zhCN } from 'date-fns/locale';
import {
  Star,
  Share,
  MoreHorizontal,
  Copy,
  Trash2,
  FileText,
  Code,
  Image as ImageIcon,
  File,
  Clock,
  HardDrive,
  Folder,
  Files,
  CheckCircle2,
  ClipboardPaste,
  ChevronRight,
} from 'lucide-react';

import { ClipboardResponseModel, ClipboardType } from '@/models/clipboardRecord.ts';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn, formatDisplayPath } from "@/lib/utils.ts";

const formatSize = (bytes?: number): string => {
  if (!bytes || bytes === 0) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  let size = bytes;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i++;
  }
  return `${size.toFixed(1)} ${units[i]}`;
};

const formatSizeLabel = (bytes?: number | null): string => {
  const value = bytes ?? 0;
  return value > 0 ? formatSize(value) : '文件夹';
};

const TypeIconMap: Record<ClipboardType, React.ElementType> = {
  [ClipboardType.Text]: FileText,
  [ClipboardType.Rtf]: Code,
  [ClipboardType.Html]: Code,
  [ClipboardType.Image]: ImageIcon,
  [ClipboardType.File]: File,
  [ClipboardType.Folder]: Folder,
};

const TypeNameMap: Record<ClipboardType, string> = {
  [ClipboardType.Text]: '文本',
  [ClipboardType.Rtf]: '富文本',
  [ClipboardType.Html]: 'HTML',
  [ClipboardType.Image]: '图片',
  [ClipboardType.File]: '文件',
  [ClipboardType.Folder]: '文件夹',
};

const FORMAT_TEXT = 'text/plain';
const FORMAT_HTML = 'text/html';
const FORMAT_RTF = 'text/rtf';

const FormatNameMap: Record<string, string> = {
  [FORMAT_TEXT]: '纯文本',
  [FORMAT_HTML]: 'HTML',
  [FORMAT_RTF]: 'RTF',
};

interface ClipboardListItemProps {
  item: ClipboardResponseModel;
  onClick?: (id: number) => void;
  onFavoriteToggle?: (id: number, current: boolean) => void;
  onShareToggle?: (id: number, current: boolean) => void;
  onCopy?: (id: number) => void;
  onCopyAs?: (id: number, format: string, asText: boolean) => void;
  onPasteAs?: (id: number, format: string, asText: boolean) => void;
  onDelete?: (id: number) => void;
  className?: string;
}

export const ClipboardListItem: React.FC<ClipboardListItemProps> = ({
  item,
  onClick,
  onFavoriteToggle,
  onShareToggle,
  onCopy,
  onCopyAs,
  onPasteAs,
  onDelete,
  className = '',
}) => {
  const {
    id,
    type,
    preview,
    size,
    sourceApp,
    createdAt,
    accessCount,
    isFavorite,
      isShared,
      isValid,
      fileItems,
      availableFormats,
  } = item;

  const TypeIcon = TypeIconMap[type] || FileText;
  const typeName = TypeNameMap[type] || '未知';
  const selectableFormats = availableFormats.filter((format) => [FORMAT_TEXT, FORMAT_HTML, FORMAT_RTF].includes(format));
  const hasMultipleFormats = selectableFormats.length > 1;

  const renderPreview = () => {
    if ((type === ClipboardType.File || type === ClipboardType.Folder) && fileItems?.length) {
      const visibleItems = fileItems.slice(0, 4);
      const hiddenCount = fileItems.length - visibleItems.length;
      const totalBytes = fileItems.reduce((sum, file) => sum + (file.size ?? 0), 0);

      return (
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant={type === ClipboardType.Folder ? 'secondary' : 'outline'} className="gap-1">
              <Files size={12} />
              {fileItems.length} 项
            </Badge>
            {totalBytes > 0 && (
              <Badge variant="outline" className="gap-1">
                <HardDrive size={12} />
                {formatSize(totalBytes)}
              </Badge>
            )}
            {isShared && <Badge className="bg-emerald-600 text-white hover:bg-emerald-600">已共享</Badge>}
          </div>

          <div className="space-y-1">
            {visibleItems.map((file) => {
              const Icon = file.isDir ? Folder : file.type === 2 ? ImageIcon : File;
              const displayPath = formatDisplayPath(file.path);
              return (
                <div
                  key={file.path}
                  className={cn(
                    'rounded-md border border-slate-200 bg-white/80 px-2 py-1.5',
                    !file.exists && 'opacity-60',
                  )}
                  title={displayPath}
                >
                  <div className="flex min-w-0 items-center gap-2">
                    <Icon size={14} className={file.isDir ? 'text-sky-500' : file.type === 2 ? 'text-violet-500' : 'text-slate-500'} />
                    <span className="min-w-0 flex-1 truncate text-xs font-medium text-slate-800">{file.name}</span>
                    <span className="shrink-0 text-[11px] text-slate-500">{file.exists ? formatSizeLabel(file.size) : '已失效'}</span>
                  </div>
                  <div className="mt-1 truncate pl-5 text-[11px] text-slate-500">{displayPath}</div>
                </div>
              );
            })}
            {hiddenCount > 0 && <div className="text-[11px] text-slate-500">还有 {hiddenCount} 项未显示</div>}
          </div>
        </div>
      );
    }

    if (!preview) {
      return <span className="italic text-slate-500">无预览</span>;
    }

    if (type === ClipboardType.Image) {
      return (
          <img
              src={`data:image/jpeg;base64,${preview}`}
              alt="预览"
              className="h-12 w-auto rounded-md bg-slate-100 object-cover"
          />
      );
    }

    // 文本预览：最多三行，保留换行符
    return (
        <div className="text-xs line-clamp-3 whitespace-pre-wrap break-words">
          {preview}
        </div>
    );
  };

  const handleFavorite = (e: React.MouseEvent) => {
    e.stopPropagation();
    onFavoriteToggle?.(id, isFavorite);
  };

  const handleShare = (e: React.MouseEvent) => {
    e.stopPropagation();
    onShareToggle?.(id, isShared);
  };

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    onCopy?.(id);
  };

  const handleCopyAs = (format: string, asText: boolean) => (e: Event) => {
    e.stopPropagation();
    onCopyAs?.(id, format, asText);
  };

  const handlePasteAs = (format: string, asText: boolean) => (e: Event) => {
    e.stopPropagation();
    onPasteAs?.(id, format, asText);
  };

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete?.(id);
  };

  return (
      <TooltipProvider delayDuration={300}>
        <Card
            className={cn(
                'fluent-card cursor-pointer overflow-hidden',
                !isValid && 'opacity-60 cursor-not-allowed',
                className
            )}
            onClick={() => isValid && onClick?.(id)}
        >
          <CardContent className="p-3">
            {/* 上下布局：上部信息区，下部预览区 */}
            <div className="flex flex-col gap-2">
              {/* 上部：类型图标 + 元信息 + 操作按钮 */}
              <div className="flex items-start gap-2">
                {/* 左侧类型图标 */}
                <div className="mt-0.5 rounded-md bg-slate-100 p-1.5 text-slate-600">
                  <TypeIcon size={14} />
                </div>

                {/* 元信息区域（自动换行，占据剩余空间） */}
                <div className="flex-1 min-w-0">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-slate-500">
                    <span className="font-medium text-slate-700">{typeName}</span>
                    {sourceApp && (
                        <Badge variant="outline" className="text-xs">
                          {sourceApp}
                        </Badge>
                    )}
                    {size && size > 0 && type !== ClipboardType.File && type !== ClipboardType.Folder && (
                        <div className="flex items-center">
                          <HardDrive size={12} className="mr-1" />
                          {formatSize(size)}
                        </div>
                    )}
                    {(type === ClipboardType.File || type === ClipboardType.Folder) && fileItems?.length ? (
                        <Badge variant="outline" className="gap-1 border-slate-200 bg-white/70 text-xs text-slate-600">
                          {isShared ? <CheckCircle2 size={11} className="text-emerald-600" /> : null}
                          {fileItems.length} 项{isShared ? ' / 已共享' : ''}
                        </Badge>
                    ) : null}
                    <div className="flex items-center gap-1">
                      <Clock size={12} />
                      <Tooltip>
                        <TooltipTrigger asChild>
                        <span>
                          {formatDistanceToNow(createdAt, {
                            addSuffix: true,
                            locale: zhCN,
                          })}
                        </span>
                        </TooltipTrigger>
                        <TooltipContent>
                          <p>{createdAt.toLocaleString()}</p>
                        </TooltipContent>
                      </Tooltip>
                    </div>
                    <span>访问 {accessCount}</span>
                  </div>
                </div>

                {/* 右侧操作按钮 */}
                <div className="flex items-center gap-1">
                  {onFavoriteToggle ? (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                            variant="ghost"
                            size="icon"
                            className="h-7 w-7 rounded-md hover:bg-slate-100"
                            onClick={handleFavorite}
                            aria-label={isFavorite ? '取消收藏' : '收藏'}
                            disabled={!isValid}
                        >
                          <Star size={14} className={isFavorite ? 'fill-yellow-500 text-yellow-500' : ''} />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>{isFavorite ? '取消收藏' : '收藏'}</p>
                      </TooltipContent>
                    </Tooltip>
                  ) : null}

                  {onShareToggle ? (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                            variant="ghost"
                            size="icon"
                            className="h-7 w-7 rounded-md hover:bg-slate-100"
                            onClick={handleShare}
                            aria-label={isShared ? '取消分享' : '分享'}
                            disabled={!isValid}
                        >
                          <Share size={14} className={isShared ? 'fill-yellow-500 text-yellow-500' : ''} />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        <p>{isShared ? '取消分享' : '分享'}</p>
                      </TooltipContent>
                    </Tooltip>
                  ) : null}

                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" className="h-7 w-7 rounded-md hover:bg-slate-100">
                        <MoreHorizontal size={14} />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      {onCopy ? (
                        <DropdownMenuItem
                            onClick={handleCopy}
                            disabled={!isValid}
                            className={!isValid ? 'opacity-50 cursor-not-allowed' : ''}
                        >
                          <Copy size={14} className="mr-2" />
                          复制内容
                        </DropdownMenuItem>
                      ) : null}
                      {hasMultipleFormats && (onPasteAs || onCopyAs) ? (
                        <>
                          <DropdownMenuSeparator />
                          {onPasteAs ? (
                            <DropdownMenuSub>
                              <DropdownMenuSubTrigger disabled={!isValid} className="justify-between">
                                <span className="flex items-center">
                                  <ClipboardPaste size={14} className="mr-2" />
                                  粘贴为
                                </span>
                                <ChevronRight size={14} />
                              </DropdownMenuSubTrigger>
                              <DropdownMenuSubContent>
                                {selectableFormats.map((format) => (
                                  <DropdownMenuItem key={`paste-${format}`} onSelect={handlePasteAs(format, false)}>
                                    {FormatNameMap[format] ?? format}
                                  </DropdownMenuItem>
                                ))}
                                {(availableFormats.includes(FORMAT_HTML) || availableFormats.includes(FORMAT_RTF)) ? (
                                  <>
                                    <DropdownMenuSeparator />
                                    <DropdownMenuItem onSelect={handlePasteAs(FORMAT_TEXT, false)}>
                                      格式转纯文本
                                    </DropdownMenuItem>
                                  </>
                                ) : null}
                              </DropdownMenuSubContent>
                            </DropdownMenuSub>
                          ) : null}
                          {onCopyAs ? (
                            <DropdownMenuSub>
                              <DropdownMenuSubTrigger disabled={!isValid} className="justify-between">
                                <span className="flex items-center">
                                  <Copy size={14} className="mr-2" />
                                  复制为
                                </span>
                                <ChevronRight size={14} />
                              </DropdownMenuSubTrigger>
                              <DropdownMenuSubContent>
                                {selectableFormats.map((format) => (
                                  <DropdownMenuItem key={`copy-${format}`} onSelect={handleCopyAs(format, format !== FORMAT_TEXT)}>
                                    {format === FORMAT_TEXT ? '纯文本' : `${FormatNameMap[format] ?? format} 源码`}
                                  </DropdownMenuItem>
                                ))}
                                {(availableFormats.includes(FORMAT_HTML) || availableFormats.includes(FORMAT_RTF)) ? (
                                  <>
                                    <DropdownMenuSeparator />
                                    <DropdownMenuItem onSelect={handleCopyAs(FORMAT_TEXT, false)}>
                                      格式转纯文本
                                    </DropdownMenuItem>
                                  </>
                                ) : null}
                              </DropdownMenuSubContent>
                            </DropdownMenuSub>
                          ) : null}
                        </>
                      ) : null}
                      {/*<DropdownMenuItem*/}
                      {/*    onClick={handleShare}*/}
                      {/*    disabled={!isValid} // 失效时禁用分享*/}
                      {/*    className={!isValid ? 'opacity-50 cursor-not-allowed' : ''}*/}
                      {/*>*/}
                      {/*  <Share size={14} className="mr-2" />*/}
                      {/*  {isShared ? '取消分享' : '分享'}*/}
                      {/*</DropdownMenuItem>*/}
                      {onDelete ? (
                        <DropdownMenuItem
                            onClick={handleDelete}
                            className="text-destructive focus:text-destructive"
                        >
                          <Trash2 size={14} className="mr-2" />
                          删除
                        </DropdownMenuItem>
                      ) : null}
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>

              {/* 下部：预览内容 */}
              <div className="relative overflow-hidden rounded-md border border-slate-100 bg-slate-50/70 p-2 text-base text-slate-900">
                {!isValid && (
                    <div className="absolute inset-0 z-10 flex items-center justify-center bg-white/55 backdrop-blur-[1px]">
                      <span className="rounded bg-white/90 px-2 py-0.5 text-xs text-slate-500">已失效</span>
                    </div>
                )}
                {renderPreview()}
              </div>
            </div>
          </CardContent>
        </Card>
      </TooltipProvider>
  );
};
