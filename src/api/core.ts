import { invoke } from "@tauri-apps/api/core"

export async function call<T>(
    command: string,
    args?: Record<string, unknown>
): Promise<T> {
    try {
        return await invoke<T>(command, args)
    } catch (error) {
        console.error(`Command failed: ${command}`, error)
        throw error
    }
}