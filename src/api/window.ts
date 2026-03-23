import {call} from "./core.ts";


// 窗口操作
export function operationWindow(operation: string, label: string) {
    return call('operation_window', {operation, label})
}