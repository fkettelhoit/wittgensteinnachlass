// Clicking the § beside a remark heading also copies the remark's full URL to the
// clipboard (the link itself still jumps to the remark, and works without this script).
// The § turns into a check mark for a moment to confirm the copy.
document.addEventListener("click", (e) => {
  const a = e.target.closest?.("a.fragment-link");
  if (!a || !navigator.clipboard) return;
  navigator.clipboard.writeText(new URL(a.getAttribute("href"), location.href).href).then(() => {
    a.textContent = "✓";
    clearTimeout(a.copiedTimer);
    a.copiedTimer = setTimeout(() => (a.textContent = "§"), 1500);
  }, () => {});
});
