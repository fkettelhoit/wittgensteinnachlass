// The colour scheme switch (layouts/partials/theme-switch.html): the system's scheme or its
// opposite, set as data-theme on <html>. The inversion is kept in localStorage, so it holds on
// other pages (applied in baseof.html's <head>) and when the system's scheme changes.
(function () {
  "use strict";

  var root = document.documentElement;
  var dark = window.matchMedia("(prefers-color-scheme: dark)");

  function apply(inverted) {
    if (inverted) root.dataset.theme = dark.matches ? "light" : "dark";
    else delete root.dataset.theme;
  }

  // On the document, as the Nachlass Map's views replace the switch with the page (js/map.js).
  document.addEventListener("click", function (e) {
    if (!e.target.closest(".theme-switch")) return;
    var inverted = !root.dataset.theme;
    apply(inverted);
    try {
      if (inverted) localStorage.setItem("theme", "inverted");
      else localStorage.removeItem("theme");
    } catch (err) {
      // no storage: the inversion holds for this page only
    }
  });

  dark.addEventListener("change", function () {
    apply(!!root.dataset.theme);
  });
})();
