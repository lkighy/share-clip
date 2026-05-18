const table = document.getElementById("table");
const empty = document.getElementById("empty");
const status = document.getElementById("status");
const detail = document.getElementById("detail");
const detailTitle = document.getElementById("detailTitle");
const detailBody = document.getElementById("detailBody");
const pagerLabel = document.getElementById("pagerLabel");
const page = Math.max(1, Number(window.__SHARE_CLIP_PAGE__) || 1);
const pageSize = 20;
let hasNextPage = false;

document.getElementById("reload").addEventListener("click", loadClipboard);
document.getElementById("close").addEventListener("click", () => detail.close());

function typeLabel(type) {
  return {0: "文本", 1: "HTML", 2: "RTF", 3: "图片", 4: "文件", 5: "文件夹"}[type] || "未知";
}

function formatSize(size) {
  if (size == null || size < 0) return "-";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = size;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}

function formatTime(seconds) {
  if (!seconds) return "-";
  return new Date(seconds * 1000).toLocaleString();
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

async function loadClipboard() {
  status.textContent = "正在加载...";
  [...table.querySelectorAll(".row")].forEach((node) => node.remove());
  empty.hidden = true;
  hasNextPage = false;
  updatePager();

  try {
    const response = await fetch(`/api/clipboard/list?page=${page}&page_size=${pageSize + 1}`, {
      headers: { "accept": "application/json" }
    });
    if (!response.ok) throw new Error(await response.text());
    const loaded = await response.json();
    hasNextPage = loaded.length > pageSize;
    const items = loaded.slice(0, pageSize);
    empty.hidden = items.length !== 0;
    for (const item of items) {
      const row = document.createElement("div");
      row.className = "row";
      row.innerHTML = `<span class="muted">${typeLabel(item.type)}</span>
        <span class="preview preview-link">${escapeHtml(item.preview || "(无预览)")}</span>
        <span class="muted hide-sm">${formatSize(item.size)}</span>
        <span class="muted hide-sm">${formatTime(item.created_at)}</span>
        <button class="button copy-button" type="button">复制</button>`;
      row.querySelector(".preview-link").addEventListener("click", () => openDetail(item));
      row.querySelector(".copy-button").addEventListener("click", (event) => {
        event.stopPropagation();
        copyItem(item, event.currentTarget);
      });
      table.append(row);
    }
    status.textContent = `第 ${page} 页，已加载 ${items.length} 条共享记录`;
    updatePager();
  } catch (error) {
    status.textContent = "加载失败";
    empty.hidden = false;
    empty.textContent = error.message || String(error);
    updatePager();
  }
}

function updatePager() {
  pagerLabel.textContent = `第 ${page} 页`;
  for (const id of ["prevTop", "prevBottom"]) {
    const link = document.getElementById(id);
    link.href = `/clipboard/${Math.max(1, page - 1)}`;
    link.classList.toggle("disabled", page <= 1);
  }
  for (const id of ["nextTop", "nextBottom"]) {
    const link = document.getElementById(id);
    link.href = `/clipboard/${page + 1}`;
    link.classList.toggle("disabled", !hasNextPage);
  }
}

async function openDetail(item) {
  detailTitle.textContent = `${typeLabel(item.type)} #${item.id}`;
  detailBody.textContent = "正在加载...";
  detail.showModal();
  try {
    const response = await fetch(`/api/clipboard/${encodeURIComponent(item.id)}/content`, {
      headers: { "accept": "application/json" }
    });
    if (!response.ok) throw new Error(await response.text());
    const content = await response.json();
    renderContent(content);
  } catch (error) {
    detailBody.innerHTML = `<pre>${escapeHtml(error.message || String(error))}</pre>`;
  }
}

async function copyItem(item, button) {
  const originalText = button.textContent;
  button.disabled = true;
  button.textContent = "复制中";
  try {
    const response = await fetch(`/api/clipboard/${encodeURIComponent(item.id)}/content`, {
      headers: { "accept": "application/json" }
    });
    if (!response.ok) throw new Error(await response.text());
    const content = await response.json();
    await writeClipboard(content);
    button.textContent = "已复制";
    setTimeout(() => {
      button.textContent = originalText;
      button.disabled = false;
    }, 1000);
  } catch (error) {
    button.textContent = "失败";
    status.textContent = `复制失败：${error.message || String(error)}`;
    setTimeout(() => {
      button.textContent = originalText;
      button.disabled = false;
    }, 1400);
  }
}

async function writeClipboard(content) {
  if (content.image_base64 && navigator.clipboard?.write && window.ClipboardItem) {
    const blob = await (await fetch(`data:image/png;base64,${content.image_base64}`)).blob();
    await navigator.clipboard.write([new ClipboardItem({ [blob.type || "image/png"]: blob })]);
    return;
  }

  const text = clipboardText(content);
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  copyTextFallback(text);
}

function copyTextFallback(text) {
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.left = "-9999px";
  document.body.append(area);
  area.select();
  const ok = document.execCommand("copy");
  area.remove();
  if (!ok) throw new Error("当前浏览器不支持剪切板写入，或页面不是安全上下文");
}

function clipboardText(content) {
  if (content.text) return content.text;
  if (content.html) return content.html;
  if (content.rtf) return content.rtf;
  if (content.files && content.files.length) return content.files.join("\n");
  if (content.image_base64) return `data:image/png;base64,${content.image_base64}`;
  return "";
}

function renderContent(content) {
  if (content.image_base64) {
    detailBody.innerHTML = `<img alt="剪切板图片" src="data:image/png;base64,${content.image_base64}">`;
    return;
  }
  if (content.files && content.files.length) {
    detailBody.innerHTML = `<ul>${content.files.map((file) => `<li>${escapeHtml(file)}</li>`).join("")}</ul>`;
    return;
  }
  const text = content.text || content.html || content.rtf || "";
  detailBody.innerHTML = `<pre>${escapeHtml(text)}</pre>`;
}

loadClipboard();
