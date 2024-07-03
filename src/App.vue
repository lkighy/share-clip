<script setup>
import {getClipText} from "./api/clipboard.js";
import {onMounted, ref} from "vue";
import ClipboardItem from "./components/ClipboardItem.vue";
import {listen, TauriEvent} from '@tauri-apps/api/event';
import {Window} from "@tauri-apps/api/window";

const data = ref("");
onMounted(async () => {
  try {
    // TODO: 聚焦在窗口时/显示窗口时，进行更新剪切板
    await listen(TauriEvent.WINDOW_FOCUS, () => {
      fetchClipText()
    })
  } catch (error) {
    console.log('调用 listen 事之后发生错误：', error)
  }
  try {
    // 当失去该窗口的聚焦时，将自动隐藏该窗口
    await listen(TauriEvent.WINDOW_BLUR, () => {
      Window.getCurrent().hide()
    })
  } catch (error) {
    console.log(error);
  }
})
function fetchClipText() {
  getClipText().then((text) => {
    data.value = text;
  }).catch((err) => {
    console.log(err);
  })
}
</script>

<template>
  <div class="container">
    <ClipboardItem :content="data" />
  </div>
</template>

<style scoped>
</style>
