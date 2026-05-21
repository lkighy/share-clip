import type { MouseEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const NON_DRAG_SELECTOR = "button,a,input,textarea,select,[data-no-drag='true']";

export async function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) {
    return;
  }

  if (event.detail > 1) {
    event.preventDefault();
    event.stopPropagation();
    return;
  }

  const target = event.target as HTMLElement;
  if (target.closest(NON_DRAG_SELECTOR)) {
    return;
  }

  event.preventDefault();
  await getCurrentWindow().startDragging();
}
