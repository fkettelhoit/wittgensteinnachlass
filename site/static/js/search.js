// Live search, inlined on the home and browse pages and on the Nachlass Map's. Hand-rolled,
// no dependencies. Queries Meilisearch directly with a read-only search key, and shows the
// hits
// - "As list": cropped, highlighted snippets in the site's normal layout (reference in the
//   left margin, text in the main column), 100 at a time, by relevance and, among equally
//   relevant hits, in the chosen ordering of the Nachlass Map (By name, By date, By format:
//   each remark's position in it is a sortable field of the index, see tools/search-index);
// - "As map" / "As bars": every remark as a block of the Nachlass Map (from /map/'s
//   data.json, styled by css/map.css), the hits marked: all of them, of each only where it
//   is (document and heading id).
// On the home page the list is the default and the map is drawn here, below the results. On
// the Nachlass Map's pages (.search-embedded, see layouts/partials/search.html) the map is
// the page's own and the default; the orderings and layouts apply to it on every view, the
// search only on the Search view, and js/map.js, which switches the views in place, keeps
// this search and announces a new map ("map:view").
(function () {
  "use strict";

  var root = document.querySelector(".search");
  if (!root) return; // no-JS message / unconfigured build — nothing to do
  var host = (root.dataset.host || "").replace(/\/+$/, "");
  var key = root.dataset.key || "";
  var index = root.dataset.index || "";
  var configured = !!(host && key && index);
  var embedded = root.classList.contains("search-embedded");
  if (!configured && !embedded) return;
  var defaultView = root.dataset.defaultView || "list";
  // By name on the home page, by date on the Nachlass Map's pages (layouts/partials/search.html).
  var defaultOrder = root.dataset.defaultOrder || (embedded ? "date" : "name");

  var input = root.querySelector(".search-input");
  var langs = root.querySelector(".search-langs");
  var langButtons = root.querySelectorAll(".search-langs button");
  var listOption = root.querySelector(".search-list");
  var viewButtons = root.querySelectorAll(".search-views button");
  var orderButtons = root.querySelectorAll(".search-orders button");
  var status = root.querySelector(".search-status");
  var results = root.querySelector(".search-results");
  var more = root.querySelector(".search-more");
  var end = root.querySelector(".search-end"); // more results, clear the search
  var mapBox = root.querySelector(".search-map"); // the home page's map (drawn here)
  var lightboxes = root.querySelector(".search-lightboxes");

  // The search is available (on the Nachlass Map only on its Search view).
  function searching() {
    return configured && root.dataset.search !== "off";
  }

  function query() {
    return input && searching() ? input.value.trim() : "";
  }

  // Private-use code points as highlight delimiters: they never occur in the corpus, so we can
  // HTML-escape the whole snippet first and only then turn them into <mark>, avoiding any
  // injection from the indexed text.
  var OPEN = String.fromCharCode(0xe000);
  var CLOSE = String.fromCharCode(0xe001);

  function esc(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c];
    });
  }

  function highlight(formatted) {
    return esc(formatted).split(OPEN).join("<mark>").split(CLOSE).join("</mark>");
  }

  function currentLang() {
    for (var i = 0; i < langButtons.length; i++) {
      if (langButtons[i].hasAttribute("aria-current")) return langButtons[i].dataset.lang;
    }
    return "de";
  }

  function currentOrder() {
    for (var i = 0; i < orderButtons.length; i++) {
      if (orderButtons[i].hasAttribute("aria-current")) return orderButtons[i].dataset.order;
    }
    return "date";
  }

  function currentView() {
    for (var i = 0; i < viewButtons.length; i++) {
      if (viewButtons[i].hasAttribute("aria-current")) return viewButtons[i].dataset.view;
    }
    return defaultView;
  }

  // The facsimile id used on the document pages, e.g. ".../Ms-101/1r.webp" -> "Ms-101-1r".
  function facId(hit) {
    var m = /\/([^/]+)\/([^/]+)\.webp(?:[?#].*)?$/.exec(hit.image_url || "");
    return m ? m[1] + "-" + m[2] : null;
  }

  // Left margin only: the works (if any) and the document name, comma-separated, linked to the
  // remark on the document page; the page reference opens the facsimile lightbox on THIS page.
  function resultHtml(hit) {
    var url = esc(hit.url);
    // Separate links: the document (→ the remark on its page), then each work it was published
    // in (→ the remark within that work, e.g. /w-rfm-3/#…). Label and URL come from the index
    // exactly as the document pages show them (e.g. "RFM III"). Joined with " &<nbsp>": the
    // break can only happen before the "&" (normal space), so "& RFM III" wraps together.
    var parts = ['<a href="' + url + '">' + esc(hit.doc) + "</a>"];
    (hit.works || []).forEach(function (w) {
      if (!w || !w.url) return; // tolerate the pre-reindex format
      parts.push('<a href="' + esc(w.url) + '">' + esc(w.label || w.code || "") + "</a>");
    });
    var refs = esc((hit.page_refs || [])[0] || "");
    var fid = facId(hit);
    var facHref = fid ? "#fac-" + esc(fid) : url;
    var snippet = highlight((hit._formatted && hit._formatted.content) || hit.content || "");
    // English results use the same muted grey as the English column on the bilingual
    // pages. Results are language-filtered, so the active language applies to all of them.
    var langClass = currentLang() === "en" ? ' class="en"' : "";
    return (
      '<article class="search-result">' +
      "<h3>" + parts.join(" &amp;&nbsp;") + "</h3>" +
      '<span class="fac"><a href="' + facHref + '">' + refs + "</a></span>" +
      "<div" + langClass + "><p>" + snippet + "</p></div>" +
      "</article>"
    );
  }

  // A self-contained facsimile lightbox (pure-CSS :target overlay, same markup as the reading
  // pages), so clicking a page reference opens the image without leaving the search page.
  function lightboxHtml(fid, imgUrl) {
    var id = esc(fid);
    var src = esc(imgUrl);
    return (
      '<div id="fac-' +
      id +
      '" class="lightbox">' +
      '<a class="lightbox-backdrop" href="#_"></a>' +
      '<a class="lightbox-close" href="#_">&times;</a>' +
      '<input type="checkbox" id="zoom-' +
      id +
      '" class="lightbox-zoom-toggle">' +
      '<label for="zoom-' +
      id +
      '" class="lightbox-zoom"><img src="' +
      src +
      '" loading="lazy"></label>' +
      "</div>"
    );
  }

  function search(body) {
    return fetch(host + "/indexes/" + encodeURIComponent(index) + "/search", {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: "Bearer " + key },
      body: JSON.stringify(body),
    }).then(function (res) {
      if (!res.ok) {
        var err = new Error("HTTP " + res.status);
        err.status = res.status;
        throw err;
      }
      return res.json();
    });
  }

  function langFilter() {
    return 'language = "' + currentLang() + '"';
  }

  // Worded like a Nachlass Map view's count of its remarks (tools/map/map.py).
  function summary(total, docs) {
    return total.toLocaleString("en") + (total === 1 ? " remark" : " remarks") +
      " in " + docs + " of " + (root.dataset.documents || "all") + " documents.";
  }

  // No search: no results, and the map (if any) without marks.
  function clear() {
    list = null;
    if (!configured) return;
    status.textContent = "";
    results.innerHTML = "";
    lightboxes.innerHTML = "";
    if (more) more.hidden = true;
    unmark();
    if (embedded) showPageMap(true);
    else mapBox.hidden = true;
    place();
  }

  // The end of the results ("Show more results", "Clear results"), only while searching:
  // after the list, or after the map. On the home page that map is drawn inside the search,
  // above the end; on the Nachlass Map it is the page's own, after the search, so the end is
  // moved below it (and back for the list). js/map.js replaces the page's map on another
  // view; the end then goes below the new one ("map:view" below). The count of the results
  // (status) stays above them, whether a list or a map.
  function place() {
    if (!end) return;
    end.hidden = !query();
    var m = embedded && currentView() !== "list" && mapEl();
    if (m) m.after(end);
    else if (end.parentNode !== root) root.insertBefore(end, lightboxes);
  }

  // --- As list -----------------------------------------------------------------------------

  var PAGE = 100;
  var list = null; // the current list: {q, page}
  var boxes = {}; // facsimile lightboxes already on the page

  // One page of the list. The first also counts the documents with hits (a facet).
  function listPage(q, page) {
    var body = {
      q: q,
      hitsPerPage: PAGE,
      page: page,
      filter: langFilter(),
      sort: ["order_" + currentOrder() + ":asc"],
      attributesToHighlight: ["content"],
      attributesToCrop: ["content"],
      cropLength: 40,
      highlightPreTag: OPEN,
      highlightPostTag: CLOSE,
    };
    if (page === 1) body.facets = ["doc"];
    return search(body).catch(function (err) {
      // An index without the orderings' fields (before its next reindex): by relevance only.
      if (err.status !== 400) throw err;
      delete body.sort;
      return search(body);
    });
  }

  function appendList(data) {
    var hits = data.hits || [];
    var lbs = "";
    hits.forEach(function (hit) {
      var fid = facId(hit);
      if (fid && !boxes[fid]) {
        boxes[fid] = 1;
        lbs += lightboxHtml(fid, hit.image_url);
      }
    });
    results.insertAdjacentHTML("beforeend", hits.map(resultHtml).join(""));
    lightboxes.insertAdjacentHTML("beforeend", lbs);
    if (more) more.hidden = data.page >= data.totalPages;
  }

  function showList(q, data) {
    if (embedded) showPageMap(false);
    else mapBox.hidden = true;
    results.hidden = false;
    results.innerHTML = "";
    lightboxes.innerHTML = "";
    boxes = {};
    list = { q: q, page: 1 };
    if (!data.totalHits) {
      status.textContent = "No results for “" + q + "”.";
      if (more) more.hidden = true;
      return;
    }
    status.textContent = summary(data.totalHits, Object.keys((data.facetDistribution || {}).doc || {}).length);
    appendList(data);
  }

  if (more) {
    more.querySelector("button").addEventListener("click", function () {
      if (!list) return;
      var mine = list;
      more.hidden = true;
      listPage(mine.q, mine.page + 1)
        .then(function (data) {
          if (list !== mine) return; // another search or ordering since
          mine.page = data.page;
          appendList(data);
        })
        .catch(function () {
          if (list === mine) status.textContent = "Search is temporarily unavailable.";
        });
    });
  }

  // Clear the search (on the home page this shows the rest of the page again, style.css)
  // and glide back to the search box, ready for the next search.
  var clearButton = root.querySelector(".search-clear button");
  if (clearButton) {
    clearButton.addEventListener("click", function () {
      input.value = "";
      syncUrl();
      runQuery("");
      var still = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      window.scrollTo({
        top: Math.max(0, input.getBoundingClientRect().top + window.scrollY - 64),
        behavior: still ? "auto" : "smooth",
      });
      input.focus({ preventScroll: true });
    });
  }

  // --- As map / As bars --------------------------------------------------------------------

  // The map: the page's own on the Nachlass Map, otherwise drawn below the results.
  function mapEl() {
    return embedded ? document.querySelector(".map-page > .map") : mapBox.querySelector(".map");
  }

  // The page's map (and what it shows, on the views that have that) is hidden for the list.
  function showPageMap(on) {
    var m = mapEl();
    if (m) m.hidden = !on;
    document.querySelectorAll(".map-page > .map-summary, .map-page > .map-terms").forEach(function (el) {
      el.hidden = !on;
    });
  }

  // The map's data and (on the home page) styles, loaded on first use. The home page's
  // blocks are built once (every remark, as on /map/); each search only marks its hits, and
  // an ordering only rearranges the documents.
  var mapData = null;
  var dataReady = null;
  var mapReady = null;
  var blocks = {}; // document -> heading id -> its block
  var marked = [];

  function loadData() {
    if (!dataReady) {
      dataReady = fetch(root.dataset.map).then(function (r) {
        if (!r.ok) throw new Error("HTTP " + r.status);
        return r.json();
      });
      dataReady.then(function (D) { mapData = D; }, function () { dataReady = null; });
    }
    return dataReady;
  }

  function loadMap() {
    if (mapReady) return mapReady;
    if (embedded) {
      mapReady = loadData().then(function (D) {
        indexBlocks(D);
        return D;
      });
    } else {
      var css = new Promise(function (resolve) {
        var link = document.createElement("link");
        link.rel = "stylesheet";
        link.href = "/css/map.css";
        link.onload = link.onerror = resolve;
        document.head.appendChild(link);
      });
      mapReady = Promise.all([loadData(), css]).then(function (all) {
        buildMap(all[0]);
        return all[0];
      });
    }
    mapReady.catch(function () { mapReady = null; }); // try again with the next search
    return mapReady;
  }

  // The markup of /map/ (tools/map/map.py, render_map): per document its sigil, then its
  // blocks in columns of four.
  function buildMap(D) {
    var out = [];
    Object.keys(D.anchors).forEach(function (doc) {
      var anchors = D.anchors[doc];
      var cols = "";
      for (var j = 0; j < anchors.length; j += 4) {
        var col = "";
        for (var i = j; i < Math.min(j + 4, anchors.length); i++) {
          col += '<i data-t="' + esc(doc + "," + D.refs[doc][i]) + '"></i>';
        }
        cols += '<span class="c">' + col + "</span>";
      }
      out.push(
        '<span class="d" data-doc="' + esc(doc) + '">' +
          '<a class="l" href="/' + esc(doc.toLowerCase()) + '/" data-t="' + esc(D.info[doc] || doc) + '">' +
          esc(doc.replace(/^(Ms|Ts)-/, "")) + "</a>" + cols + "</span>"
      );
    });
    mapBox.innerHTML = '<div class="map js">' + out.join(" ") + "</div>";
    indexBlocks(D);
    layout();
    arrange(currentOrder());
  }

  function indexBlocks(D) {
    blocks = {};
    marked = [];
    var m = mapEl();
    if (!m) return;
    m.querySelectorAll(".d").forEach(function (d) {
      var doc = d.dataset.doc;
      var ids = D.anchors[doc] || [];
      var byId = (blocks[doc] = {});
      d.querySelectorAll(".c > i").forEach(function (b, i) {
        byId[ids[i]] = b;
      });
    });
  }

  // The map's layout: documents running on like text (As map) or one per line (As bars).
  function layout() {
    var m = mapEl();
    if (m) m.classList.toggle("bars", currentView() === "bars");
  }

  // Arrange the documents in an ordering, a group's label (a year, Manuscripts/Typescripts,
  // a format) before its first document (the static pages are in date order).
  function arrange(order) {
    var m = mapEl();
    if (!m || !mapData) return;
    var docs = {};
    m.querySelectorAll(".d").forEach(function (d) { docs[d.dataset.doc] = d; });
    m.querySelectorAll(".g").forEach(function (g) { g.remove(); });
    var frag = document.createDocumentFragment();
    var long = false;
    mapData.orders[order].forEach(function (group) {
      group[1].forEach(function (name, k) {
        var d = docs[name];
        if (!d) return;
        if (k === 0) {
          var g = document.createElement("b");
          g.className = "g";
          g.textContent = group[0];
          if (group[0].length > 4) {
            g.className += " long"; // rotated along the divider in the bars layout
            long = true;
          }
          d.insertBefore(g, d.firstChild);
        }
        frag.appendChild(d);
        frag.appendChild(document.createTextNode(" "));
      });
    });
    m.textContent = "";
    m.appendChild(frag);
    m.classList.toggle("long-groups", long);
  }

  function rearrange() {
    if (embedded && currentOrder() === "date" && !mapData) return; // as the page is
    loadMap().then(function () { arrange(currentOrder()); });
  }

  if (mapBox) {
    // A block links to its remark, a sigil to its document, in the language searched (on the
    // Nachlass Map's pages, js/map.js does this).
    mapBox.addEventListener("click", function (e) {
      var a = e.target.closest(".l");
      if (a) {
        if (currentLang() === "en") a.href = "/en" + a.getAttribute("href").replace(/^\/en/, "");
        return;
      }
      var b = e.target.closest(".c > i");
      if (!b || !mapData) return;
      var d = b.closest(".d");
      var i = Array.prototype.indexOf.call(d.querySelectorAll(".c > i"), b);
      location.href = (currentLang() === "en" ? "/en/" : "/") + d.dataset.doc.toLowerCase() + "/#" + mapData.anchors[d.dataset.doc][i];
    });
    // Popovers near the right edge of the window open to the left (as in js/map.js).
    mapBox.addEventListener("mouseover", function (e) {
      var el = e.target.closest(".c > i, .l");
      if (el) el.classList.toggle("left", el.getBoundingClientRect().left > window.innerWidth - 420);
    });
  }

  // Every hit (the index's maxTotalHits is above the number of remarks, see
  // tools/search-index), and of each only where it is: no snippets or highlights.
  function allHits(q) {
    return search({ q: q, limit: 200000, filter: langFilter(), attributesToRetrieve: ["doc", "fragment"] });
  }

  function unmark() {
    marked.forEach(function (b) {
      b.classList.remove("s10", "hit");
    });
    marked = [];
  }

  function showMap(q, data) {
    unmark();
    var hits = data.hits || [];
    var docs = {};
    hits.forEach(function (hit) {
      docs[hit.doc] = 1;
      var b = (blocks[hit.doc] || {})[hit.fragment];
      if (!b) return;
      b.classList.add("s10", "hit");
      marked.push(b);
    });
    list = null;
    results.hidden = true;
    results.innerHTML = "";
    lightboxes.innerHTML = "";
    if (more) more.hidden = true;
    if (embedded) showPageMap(true);
    else mapBox.hidden = false;
    status.textContent = hits.length ? summary(hits.length, Object.keys(docs).length) : "No results for “" + q + "”.";
  }

  // --- Searching ---------------------------------------------------------------------------

  var latest = 0;
  function runQuery() {
    var q = query();
    place();
    if (!q) {
      latest++; // cancel any in-flight render
      clear();
      return;
    }
    var seq = ++latest;
    var asMap = root.dataset.map && currentView() !== "list";
    var request = asMap
      ? Promise.all([allHits(q), loadMap()]).then(function (r) { return r[0]; })
      : listPage(q, 1);
    request
      .then(function (data) {
        if (seq !== latest) return;
        if (asMap) showMap(q, data);
        else showList(q, data);
      })
      .catch(function () {
        if (seq === latest) {
          status.textContent = "Search is temporarily unavailable.";
          results.innerHTML = "";
          if (more) more.hidden = true;
        }
      });
  }

  // The current option of a switcher (style.css: .switch[aria-current]).
  function setCurrent(button, on) {
    if (on) button.setAttribute("aria-current", "true");
    else button.removeAttribute("aria-current");
  }

  function setLang(lang) {
    for (var j = 0; j < langButtons.length; j++) {
      setCurrent(langButtons[j], langButtons[j].dataset.lang === lang);
    }
  }

  function setView(view) {
    for (var j = 0; j < viewButtons.length; j++) {
      setCurrent(viewButtons[j], viewButtons[j].dataset.view === view);
    }
  }

  function setOrder(order) {
    for (var j = 0; j < orderButtons.length; j++) {
      setCurrent(orderButtons[j], orderButtons[j].dataset.order === order);
    }
  }

  // Show what the current view offers: on the Nachlass Map's views other than Search, only
  // the orderings and the layouts (not the search box, its languages or the list).
  function applyMode() {
    var on = searching();
    [input, langs, listOption].forEach(function (el) {
      if (el) el.hidden = !on;
    });
    if (!on && currentView() === "list") setView(defaultView);
  }

  // Keep the query, the language, the ordering and the view in the URL (like any search
  // parameter), so a search is fully shareable; the query and language only with a query.
  function syncUrl() {
    var url = new URL(window.location.href);
    var q = query();
    if (q) {
      url.searchParams.set("q", q);
      url.searchParams.set("lang", currentLang());
    } else {
      url.searchParams.delete("q");
      url.searchParams.delete("lang");
    }
    if (currentOrder() !== defaultOrder) url.searchParams.set("order", currentOrder());
    else url.searchParams.delete("order");
    if (currentView() !== defaultView) url.searchParams.set("view", currentView());
    else url.searchParams.delete("view");
    // On the home page a bare ordering or view (without a query) is not worth a URL.
    if (!q && !embedded) {
      url.searchParams.delete("order");
      url.searchParams.delete("view");
    }
    window.history.replaceState(history.state, "", url);
  }

  if (input) {
    var timer;
    input.addEventListener("input", function () {
      clearTimeout(timer);
      timer = setTimeout(function () {
        syncUrl();
        runQuery();
      }, 180);
    });
  }

  for (var i = 0; i < langButtons.length; i++) {
    langButtons[i].addEventListener("click", function () {
      if (this.hasAttribute("aria-current")) return;
      setLang(this.dataset.lang);
      syncUrl();
      runQuery();
    });
  }

  for (var k = 0; k < viewButtons.length; k++) {
    viewButtons[k].addEventListener("click", function () {
      if (this.hasAttribute("aria-current")) return;
      var wasList = currentView() === "list";
      setView(this.dataset.view);
      syncUrl();
      layout();
      // Between map and bars only the layout changes.
      if (wasList || this.dataset.view === "list") runQuery();
    });
  }

  for (var o = 0; o < orderButtons.length; o++) {
    orderButtons[o].addEventListener("click", function () {
      if (this.hasAttribute("aria-current")) return;
      setOrder(this.dataset.order);
      syncUrl();
      // The map's documents are rearranged (its marks stay); the list is searched again.
      if (currentView() === "list" && query()) runQuery();
      else rearrange();
    });
  }

  // Another view of the Nachlass Map (js/map.js): its map in the current ordering and layout,
  // and on the Search view with the current search's marks.
  document.addEventListener("map:view", function () {
    mapReady = null;
    blocks = {};
    marked = [];
    applyMode();
    layout();
    rearrange();
    syncUrl();
    if (query()) runQuery();
    else clear();
  });

  // Reveal the UI (it is hidden so the no-JS message shows when JS is unavailable).
  root.hidden = false;
  var params = new URLSearchParams(window.location.search);
  var initialLang = params.get("lang");
  if (initialLang === "de" || initialLang === "en") setLang(initialLang);
  var initialOrder = params.get("order");
  if (initialOrder === "name" || initialOrder === "date" || initialOrder === "format") setOrder(initialOrder);
  var initialView = params.get("view");
  for (var v = 0; v < viewButtons.length; v++) {
    if (viewButtons[v].dataset.view === initialView) setView(initialView);
  }
  applyMode();
  if (embedded) {
    layout();
    rearrange();
  }
  if (input && searching() && params.get("q")) {
    input.value = params.get("q");
    runQuery();
  }
})();
