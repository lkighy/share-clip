import React, { useState, useRef, useEffect, KeyboardEvent } from 'react';

interface HotkeyInputProps {
    value: string;           // 当前快捷键字符串，如 "Ctrl+F1"
    onChange: (value: string) => void;
    placeholder?: string;
    className?: string;
}

const HotkeyInput: React.FC<HotkeyInputProps> = ({
                                                     value,
                                                     onChange,
                                                     placeholder = '按下快捷键',
                                                     className = '',
                                                 }) => {
    const [isCapturing, setIsCapturing] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);

    // 进入捕获模式
    const startCapture = () => {
        setIsCapturing(true);
    };

    // 退出捕获模式
    const stopCapture = () => {
        setIsCapturing(false);
    };

    // 格式化快捷键字符串
    const formatHotkey = (event: KeyboardEvent): string => {
        const keys: string[] = [];
        if (event.ctrlKey) keys.push('Ctrl');
        if (event.shiftKey) keys.push('Shift');
        if (event.altKey) keys.push('Alt');
        if (event.metaKey) keys.push('Meta'); // Mac 的 Command 键

        // 获取主键名（忽略重复的修饰键）
        let mainKey = event.key;
        // 处理特殊键名，如 " " -> "Space"
        if (mainKey === ' ') mainKey = 'Space';
        // 避免显示像 "Control" 这样的重复信息
        if (mainKey === 'Control' || mainKey === 'Shift' || mainKey === 'Alt' || mainKey === 'Meta') {
            // 如果只按了修饰键，不记录（等待下一个键）
            return '';
        }
        // 将主键标准化（例如 "ArrowUp" -> "Up"）
        if (mainKey.startsWith('Arrow')) mainKey = mainKey.slice(5);
        if (mainKey === 'Escape') return ''; // Esc 用于取消，不记录

        keys.push(mainKey);
        return keys.join('+');
    };

    // 全局键盘监听
    useEffect(() => {
        if (!isCapturing) return;

        const handleKeyDown = (e: globalThis.KeyboardEvent) => {
            e.preventDefault();       // 阻止浏览器默认行为（如 Ctrl+F 打开查找）
            e.stopPropagation();

            // 按下 Esc 取消捕获
            if (e.key === 'Escape') {
                stopCapture();
                return;
            }

            // 将事件转换成 React 兼容的 KeyboardEvent 对象（用于调用 formatHotkey）
            const syntheticEvent = e as unknown as KeyboardEvent;
            const hotkeyStr = formatHotkey(syntheticEvent);
            if (hotkeyStr) {
                onChange(hotkeyStr);
                stopCapture();
            }
            // 如果只按了修饰键，等待下一次按键（不退出捕获）
        };

        window.addEventListener('keydown', handleKeyDown);
        return () => window.removeEventListener('keydown', handleKeyDown);
    }, [isCapturing, onChange]);

    // 处理 input 获得焦点
    const handleFocus = () => {
        startCapture();
    };

    // 处理失去焦点（取消捕获）
    const handleBlur = () => {
        stopCapture();
    };

    return (
        <input
            ref={inputRef}
            type="text"
            className={className}
            value={isCapturing ? '按下快捷键...' : value}
            onFocus={handleFocus}
            onBlur={handleBlur}
            placeholder={placeholder}
            readOnly  // 禁止手动输入
        />
    );
};

export default HotkeyInput;