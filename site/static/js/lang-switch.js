// The home page's switcher between its German and German/English version, on the line of
// the document lists' switcher (partials/home.html): it opens the other version with the
// same list shown (by name, date or format) and at the same document, as the lists and
// their anchors are the same in both. Without JS it opens the list by name.
document.addEventListener("click", (e) => {
  const a = e.target.closest("a[data-keep-view]");
  if (!a || !location.hash) return;
  const target = document.getElementById(decodeURIComponent(location.hash.slice(1)));
  if (target && (/^by-/.test(target.id) || target.closest(".doc-list"))) {
    a.href = a.pathname + location.hash;
  }
});
