<script setup>
// This starter template is using Vue 3 <script setup> SFCs
// Check out https://vuejs.org/api/sfc-script-setup.html#script-setup
import {get_clip_text} from "./api/clipboard.js";
import {onMounted, onUnmounted, ref} from "vue";
import ClipboardItem from "./components/ClipboardItem.vue";
import {Window} from "@tauri-apps/api/window";
import {WebviewWindow} from "@tauri-apps/api/webviewWindow";
import {listen, TauriEvent} from "@tauri-apps/api/event";

const data = ref("");
const info = ref('没触发');
onMounted(async () => {
  const focus = await WebviewWindow.getCurrent().once(TauriEvent.WINDOW_FOCUS, () => {
    console.log('触发 focus')
  })
  console.log('什么内容');
  focus()
  // await WebviewWindow.getCurrent().listen('focus', (event) => {
  //   info.value = 'WebviewWindow 触发 focus';
  // })
})
onUnmounted(() => {
  info.value = '触发 onUnmounted';

})
</script>

<template>
  <div class="container">
    <ClipboardItem :content="data" />
    {{ info }}
  </div>
</template>

<style scoped>
</style>
