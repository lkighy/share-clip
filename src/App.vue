<script setup>
// This starter template is using Vue 3 <script setup> SFCs
// Check out https://vuejs.org/api/sfc-script-setup.html#script-setup
import Greet from "./components/Greet.vue";
import { get_clip_text } from "./api/clipboard.js";
import { ref, onMounted } from "vue";

const data = ref("");
onMounted(async () => {
    try {
        const response = await get_clip_text();
        data.value = response;

    } catch (error) {
      this.$message.error(error)
    }
})
</script>

<template>
  <div class="container">
    {{ data }}
  </div>
</template>

<style scoped>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.vue:hover {
  filter: drop-shadow(0 0 2em #249b73);
}
</style>
