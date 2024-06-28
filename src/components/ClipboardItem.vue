<script setup>
const props = defineProps({
  content: {
    type: String,
    required: true,
  },
})
</script>

<template>
<div class="clipboard-item">
  <p v-text="content"></p>
</div>
</template>

<style scoped>
.clipboard-item {
  padding: 10px;
  border-bottom: 1px solid #ccc;
}
.clipboard-item p {
  font-size: 12px;
  text-align: left;
  margin: 0;
  word-break: break-all;
}
</style>