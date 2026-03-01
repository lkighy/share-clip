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
} from 'lucide-react';

import { ClipboardResponseModel, ClipboardType } from '@/models/clipboardRecord.ts';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent } from '@/components/ui/card';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

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

interface ClipboardListItemProps {
  item: ClipboardResponseModel;
  onClick?: (id: number) => void;
  onFavoriteToggle?: (id: number, current: boolean) => void;
  onShareToggle?: (id: number, current: boolean) => void;
  onCopy?: (id: number) => void;
  onDelete?: (id: number) => void;
  className?: string;
}

export const ClipboardListItem: React.FC<ClipboardListItemProps> = ({
  item,
  onClick,
  onFavoriteToggle,
  onShareToggle,
  onCopy,
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
  } = item;

  const TypeIcon = TypeIconMap[type] || FileText;
  const typeName = TypeNameMap[type] || '未知';

  const renderPreview = () => {
    if (!preview) {
      return <span className="text-muted-foreground italic">无预览</span>;
    }

    if (type === ClipboardType.Image) {
      return (
          <img
              src={`data:image/jpeg;base64,${preview}`}
              alt="预览"
              className="h-12 w-auto rounded object-cover bg-muted"
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

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation();
    onDelete?.(id);
  };

  return (
      <TooltipProvider delayDuration={300}>
        <Card
            className={`cursor-pointer transition-colors hover:bg-accent/5 ${className}`}
            onClick={() => onClick?.(id)}
        >
          <CardContent className="p-3">
            {/* 上下布局：上部信息区，下部预览区 */}
            <div className="flex flex-col gap-2">
              {/* 上部：类型图标 + 元信息 + 操作按钮 */}
              <div className="flex items-start gap-2">
                {/* 左侧类型图标 */}
                <div className="mt-0.5 text-muted-foreground">
                  <TypeIcon size={14} />
                </div>

                {/* 元信息区域（自动换行，占据剩余空间） */}
                <div className="flex-1 min-w-0">
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                    <span>{typeName}</span>
                    {sourceApp && (
                        <Badge variant="outline" className="text-xs">
                          {sourceApp}
                        </Badge>
                    )}
                    {size && size > 0 && (
                        <div className="flex items-center">
                          <HardDrive size={12} className="mr-1" />
                          {formatSize(size)}
                        </div>
                    )}
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
                  {/* 收藏按钮 */}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                          variant="ghost"
                          size="icon"
                          className="h-6 w-6"
                          onClick={handleFavorite}
                          aria-label={isFavorite ? '取消收藏' : '收藏'}
                      >
                        <Star
                            size={14}
                            className={isFavorite ? 'fill-yellow-500 text-yellow-500' : ''}
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      <p>{isFavorite ? '取消收藏' : '收藏'}</p>
                    </TooltipContent>
                  </Tooltip>

                  {/* 更多操作菜单 */}
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" className="h-6 w-6">
                        <MoreHorizontal size={14} />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem onClick={handleCopy}>
                        <Copy size={14} className="mr-2" />
                        复制内容
                      </DropdownMenuItem>
                      <DropdownMenuItem onClick={handleShare}>
                        <Share size={14} className="mr-2" />
                        {isShared ? '取消分享' : '分享'}
                      </DropdownMenuItem>
                      <DropdownMenuItem
                          onClick={handleDelete}
                          className="text-destructive focus:text-destructive"
                      >
                        <Trash2 size={14} className="mr-2" />
                        删除
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>

              {/* 下部：预览内容 */}
              <div className="rounded-md bg-muted/30 p-2 text-base text-foreground/90 overflow-hidden">
                {renderPreview()}
              </div>
            </div>
          </CardContent>
        </Card>
      </TooltipProvider>
  );
};
