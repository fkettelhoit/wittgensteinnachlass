// Live search, inlined on the home and browse pages. Hand-rolled, no dependencies. Queries
// Meilisearch directly with a read-only search key and renders cropped, highlighted snippets
// in the site's normal layout (reference in the left margin, text in the main column).
(function () {
  "use strict";

  var root = document.querySelector(".search");
  if (!root) return; // no-JS message / unconfigured build — nothing to do
  var host = (root.dataset.host || "").replace(/\/+$/, "");
  var key = root.dataset.key || "";
  var index = root.dataset.index || "";
  if (!host || !key || !index) return;

  var input = root.querySelector(".search-input");
  var langButtons = root.querySelectorAll(".search-langs button");
  var status = root.querySelector(".search-status");
  var results = root.querySelector(".search-results");
  var lightboxes = root.querySelector(".search-lightboxes");

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
      if (langButtons[i].classList.contains("lang-active")) return langButtons[i].dataset.lang;
    }
    return "de";
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
    return (
      '<article class="search-result">' +
      "<h3>" + parts.join(" &amp;&nbsp;") + "</h3>" +
      '<span class="fac"><a href="' + facHref + '">' + refs + "</a></span>" +
      "<div><p>" + snippet + "</p></div>" +
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

  function escRe(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }

  // Exact = the query occurs as a whole word/phrase (Unicode-aware, so "ich" is not exact
  // inside "nicht"). Used only to group results, not to filter them.
  function isExact(hit, q) {
    var content = hit.content || "";
    try {
      return new RegExp("(^|[^\\p{L}\\p{N}])" + escRe(q) + "([^\\p{L}\\p{N}]|$)", "iu").test(
        content,
      );
    } catch (e) {
      return content.toLowerCase().indexOf(q.toLowerCase()) !== -1;
    }
  }

  function render(data, q) {
    var hits = (data.hits || []).slice();
    if (!hits.length) {
      status.textContent = "No results for “" + q + "”.";
      results.innerHTML = "";
      lightboxes.innerHTML = "";
      return;
    }
    // Exact matches first; within each group, document + page order (the indexer's `ord`).
    for (var i = 0; i < hits.length; i++) hits[i]._exact = isExact(hits[i], q);
    hits.sort(function (a, b) {
      if (a._exact !== b._exact) return a._exact ? -1 : 1;
      return (a.ord || 0) - (b.ord || 0);
    });
    var total = data.estimatedTotalHits || hits.length;
    status.textContent =
      total > hits.length
        ? "Showing the first " + hits.length + " of about " + total + " results."
        : "Found " + hits.length + (hits.length === 1 ? " result." : " results.");
    results.innerHTML = hits.map(resultHtml).join("");

    var seen = {};
    var boxes = "";
    for (var j = 0; j < hits.length; j++) {
      var fid = facId(hits[j]);
      if (!fid || seen[fid]) continue;
      seen[fid] = 1;
      boxes += lightboxHtml(fid, hits[j].image_url);
    }
    lightboxes.innerHTML = boxes;
  }

  function clear() {
    status.textContent = "";
    results.innerHTML = "";
    lightboxes.innerHTML = "";
  }

  var latest = 0;
  function runQuery(q) {
    q = q.trim();
    if (!q) {
      latest++; // cancel any in-flight render
      clear();
      return;
    }
    var seq = ++latest;
    var body = {
      q: q,
      limit: 100,
      filter: 'language = "' + currentLang() + '"',
      attributesToHighlight: ["content"],
      attributesToCrop: ["content"],
      cropLength: 40,
      highlightPreTag: OPEN,
      highlightPostTag: CLOSE,
    };

    fetch(host + "/indexes/" + encodeURIComponent(index) + "/search", {
      method: "POST",
      headers: { "Content-Type": "application/json", Authorization: "Bearer " + key },
      body: JSON.stringify(body),
    })
      .then(function (res) {
        if (!res.ok) throw new Error("HTTP " + res.status);
        return res.json();
      })
      .then(function (data) {
        if (seq === latest) render(data, q);
      })
      .catch(function () {
        if (seq === latest) {
          status.textContent = "Search is temporarily unavailable.";
          results.innerHTML = "";
        }
      });
  }

  function setLang(lang) {
    for (var j = 0; j < langButtons.length; j++) {
      langButtons[j].classList.toggle("lang-active", langButtons[j].dataset.lang === lang);
    }
  }

  // Keep both the query and the language in the URL (like any search parameter), so a search
  // is fully shareable; drop both when the query is empty.
  function syncUrl() {
    var url = new URL(window.location.href);
    var q = input.value.trim();
    if (q) {
      url.searchParams.set("q", q);
      url.searchParams.set("lang", currentLang());
    } else {
      url.searchParams.delete("q");
      url.searchParams.delete("lang");
    }
    window.history.replaceState(null, "", url);
  }

  var timer;
  input.addEventListener("input", function () {
    clearTimeout(timer);
    timer = setTimeout(function () {
      syncUrl();
      runQuery(input.value);
    }, 180);
  });
  for (var i = 0; i < langButtons.length; i++) {
    langButtons[i].addEventListener("click", function () {
      if (this.classList.contains("lang-active")) return;
      setLang(this.dataset.lang);
      syncUrl();
      runQuery(input.value);
    });
  }

  // Reveal the UI (it is hidden so the no-JS message shows when JS is unavailable).
  root.hidden = false;
  var params = new URLSearchParams(window.location.search);
  var initialLang = params.get("lang");
  if (initialLang === "de" || initialLang === "en") setLang(initialLang);
  var initialQ = params.get("q") || "";
  if (initialQ) {
    input.value = initialQ;
    runQuery(initialQ);
  }
})();
