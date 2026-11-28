// Clicking the § beside a remark (or the ¶ beside a section heading) also copies its full
// URL to the clipboard (the link itself still jumps to the heading, and works without this
// script).
document.addEventListener("click", (e) => {
  const a = e.target.closest?.("a.fragment-link");
  if (!a || !navigator.clipboard) return;
  navigator.clipboard.writeText(new URL(a.getAttribute("href"), location.href).href).catch(() => {});
});
