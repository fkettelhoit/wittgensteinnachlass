// Nachlass Map (/map/): optional enhancements of the static pages written by tools/map —
// links from the blocks to the remarks, popovers that open to the left near the right edge,
// and switching between the views (search, themes, published, works) in place, without
// loading a new page. The search, the orderings and the layouts are js/search.js's; when
// the view changes, the search (with its query and switchers) stays as it is and search.js
// is told to apply them to the new map ("map:view" event). Without JavaScript the map shows
// the date order, with popovers, and the views are plain links.
(function () {
  "use strict";

  var page = document.querySelector(".map-page");
  if (!page || !page.querySelector(".map") || !page.dataset.src) return;

  // The remarks' heading ids (the same for every view), loaded on first use.
  var data = null;
  function load() {
    if (!data) data = fetch(page.dataset.src).then(function (r) { return r.json(); });
    return data;
  }

  // The URL of another view: its path, with the ordering and the view of the map (but not the
  // query, which only the Search view shows).
  function href(path) {
    var params = new URLSearchParams(location.search);
    var keep = new URLSearchParams();
    ["order", "view"].forEach(function (k) {
      if (params.get(k)) keep.set(k, params.get(k));
    });
    var q = keep.toString();
    return path + (q ? "?" + q : "");
  }

  // Switch to another view in place: replace the page's content with the other page's but
  // keep the search, keep the scroll position, and change the URL (back and forward switch
  // back).
  var pending = 0;
  var shown = location.pathname; // the view on the page
  function go(path, push) {
    var request = ++pending;
    var url = push ? href(path) : location.pathname + location.search;
    fetch(path)
      .then(function (r) {
        if (!r.ok) throw new Error(r.status);
        return r.text();
      })
      .then(function (html) {
        if (request !== pending) return; // a later switch wins
        var doc = new DOMParser().parseFromString(html, "text/html");
        var main = doc.querySelector("main.map-page");
        if (!main) throw new Error("no map");
        var search = page.querySelector(".search");
        var next = main.querySelector(".search");
        if (search && next) {
          if (next.dataset.search) search.dataset.search = next.dataset.search;
          else delete search.dataset.search;
          next.replaceWith(search);
        }
        page.replaceChildren.apply(page, Array.prototype.slice.call(main.childNodes));
        document.title = doc.title;
        shown = path;
        if (push) history.pushState({ map: true }, "", url);
        document.dispatchEvent(new CustomEvent("map:view"));
      })
      .catch(function () {
        location.href = url; // fall back to loading the page
      });
  }

  page.addEventListener("click", function (e) {
    var a = e.target.closest("a");
    // Another view (unless opened in a new tab or window).
    if (a && a.closest(".map-views")) {
      if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      e.preventDefault();
      if (a.pathname !== location.pathname) go(a.pathname, true);
      return;
    }
    // A block links to its remark on the document page (the English one while searching in
    // English).
    var cell = e.target.closest(".map-page > .map .c > i");
    if (!cell) return;
    var d = cell.closest(".d");
    var i = Array.prototype.indexOf.call(d.querySelectorAll(".c > i"), cell);
    var en = page.querySelector(".search:not([data-search]) .search-langs [aria-current][data-lang=en]");
    load().then(function (D) {
      var id = (D.anchors[d.dataset.doc] || [])[i];
      location.href = (en ? "/en/" : "/") + d.dataset.doc.toLowerCase() + "/" + (id ? "#" + id : "");
    });
  });

  // Back and forward between views. A fragment navigation (opening or closing a facsimile
  // lightbox, #fac-…) also fires popstate; on the same view it must not reload the view,
  // which would re-run the search and remove the lightboxes it made.
  window.addEventListener("popstate", function () {
    if (location.pathname === shown) return;
    go(location.pathname, false);
  });

  // Popovers (map.css) near the right edge of the window open to the left.
  page.addEventListener("mouseover", function (e) {
    var el = e.target.closest(".map .c > i, .map .l");
    if (el) el.classList.toggle("left", el.getBoundingClientRect().left > window.innerWidth - 420);
  });

  // Blocks are links (a pointer cursor) only with the script.
  page.querySelector(".map").classList.add("js");
  document.addEventListener("map:view", function () {
    var m = page.querySelector(".map");
    if (m) m.classList.add("js");
  });
  history.replaceState({ map: true }, "", location.href);
})();
