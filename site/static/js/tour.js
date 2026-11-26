// Guided tour of the site: a popover that walks through the main features step by
// step, highlighting the element to look at, marking the one to click, and advancing
// once the reader has done so. Self-contained and inert unless a tour is in progress:
// it is started by the button on the homepage (partials/intro.html, a link to
// "/?tour": the tour follows the German pages, from the German home page), and the current step is kept in sessionStorage so the tour survives
// navigating between pages (and is forgotten when the tab is closed). On a page
// other than the current step's, the tour stays out of the way: it pauses, and
// picks up again on returning to that page (e.g. with the back button).
//
// Keys: Enter and → do what "Next" does, ← goes back a step (to the previous page,
// if the step before is on another one).
//
// The highlight and the popover are positioned in page coordinates, so they scroll
// with the content. The popover always sits right below the highlight, on every
// screen size. Moving to a step on the same page animates both boxes (CSS
// transitions) while a single smooth scroll brings them into view; whenever the
// highlight moves or changes size afterwards (web fonts or images loading, a
// <details> opening), they are scrolled into view again.
(() => {
  const KEY = "tour-step";
  const BACK = "tour-back"; // set while going back to a step on another page
  const URLS = "tour-urls"; // the URL each page was left at, to go back to it
  const ANIMATION = 400; // ms, matches the transitions in style.css
  const PAD = 6, GAP = 12, MARGIN = 16;

  // --- Steps ------------------------------------------------------------------
  // page: pathname the step lives on, or several (the first is where going back to
  // the step loads it), e.g. a Nachlass Map view, which js/map.js switches to in
  // place, without loading the page. target: element(s) to highlight (the highlight
  // spans the content of all matches, e.g. a remark's margin columns and text): a
  // selector, or a function returning the elements.
  // click: advance when this is clicked ("Next" clicks it, if visible). cue:
  // element(s) to mark as the thing to interact with (default: click); a function
  // is re-evaluated whenever the page changes. done: advance once this is true
  // (checked whenever the page changes). next: what "Next" does otherwise.
  // skip: leave the step out when this is true (checked on its own page).
  // undo: going back from this step, revert what the reader did to finish the step
  // before (on the same page), so that it doesn't count as done right away.
  // fallback: not counted in "n of m" (it stands in for skipped steps).
  // continues: part of the step before (e.g. what its click opened): the same number in
  // "n of m" and the same title, so that the count is one per topic.
  // final: the closing message: not counted, no "n of m", and "Finish"; the start
  // button is shown again for it.
  // boxes: target elements measured by their whole box (and ::after) rather than
  // their content, e.g. the sticky title bar with its dividing line, or a bubble.
  // fixed: the target includes the sticky title bar: keep the bar shown, scroll
  // only as far down as needed to show the popover, and follow the target while
  // the page scrolls.
  // text: paragraphs separated by blank lines.
  //
  // The route follows the motto of the Philosophical Investigations, which quotes
  // Nestroy: search → PI → its facsimile → a later version in Ms-134 → Ms-134's
  // similar remarks → Ts-229, where many of them were reworked → back home to the
  // Nachlass Map: the same search there, its hits on the map → the Mathematics view.

  const HOME = "/"; // the German home page, which the German pages link to
  const PI = "/w-pi/";
  const MOTTO = "ts-227a-1-2"; // the motto's id in PI
  const MOTTO_SECTION = `section:has(> [id="${MOTTO}"])`;
  const VERSIONS = `[id="${MOTTO}"] .versions`;
  const TO_MS134 = `${VERSIONS} a[href="/ms-134/#152-3"]`;
  const RESULT_LINK = `a[href="${PI}#${MOTTO}"]`; // the search result's "PI" link
  const RESULT = `.search-results ${RESULT_LINK}`;
  const MS134 = "/ms-134/";
  const MAP = "/map/"; // the Nachlass Map's Search view
  const MATH = "/map/themes/math/"; // its Mathematics view (js/map.js switches to it in place)
  const MOTTO_DOCS = ["Ts-227a", "Ts-227b", "Ms-134"]; // where the search finds the motto
  const SIMILAR = "main > details.similar";
  const TS229 = `${SIMILAR} > details:has(> summary a[href="/ts-229/"])`;
  const searchInput = () => document.querySelector(".search:not([hidden]) .search-input");
  const clearSearch = () => {
    const input = searchInput();
    input.value = "";
    input.dispatchEvent(new Event("input", { bubbles: true }));
  };

  const steps = [
    {
      page: HOME,
      // The note on the works compiled after Wittgenstein's death and their first row of
      // covers (not all of them on narrow screens, where they wrap, so that the highlight
      // and the popover fit together).
      target: () => {
        const note = [...document.querySelectorAll("main > .section-note")].find((p) =>
          p.textContent.startsWith("Compiled by"),
        );
        // The covers follow the note and the switcher between the German and the
        // German/English home page.
        let grid = note?.nextElementSibling;
        while (grid && !grid.querySelector("a.cover")) grid = grid.nextElementSibling;
        const covers = [...(grid?.querySelectorAll("a.cover") ?? [])];
        const row = covers.filter((el) => el.offsetTop === covers[0]?.offsetTop);
        return [note, ...row].filter(Boolean);
      },
      title: "Writings and Documents",
      text: "These writings were compiled by Wittgenstein’s literary executors after his death. Above is the <em>Tractatus</em>, which he published himself. Further down: his manuscripts and typescripts, by name, date or format (e.g. notebook or loose sheets).",
    },
    {
      page: HOME,
      target: ".search-input",
      cue: ".search-input",
      skip: () => !searchInput(),
      // Wait until the reader has typed most of the word, not just its first letter.
      done: () => /^nestr/i.test(searchInput()?.value.trim() ?? "") && document.querySelector(RESULT),
      next: () => {
        const input = searchInput();
        input.value = "Nestroy";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      },
      title: "Search",
      text: "You can also search all transcriptions at once. While you search, the rest of the home page is hidden; clear the search to bring it back.\n\nTry it: search for <strong>Nestroy</strong>.",
    },
    {
      page: HOME,
      target: `.search-result:has(${RESULT_LINK})`,
      click: RESULT,
      skip: () => !searchInput(),
      undo: clearSearch,
      continues: true,
      title: "Search",
      text: "The motto of the <em>Philosophical Investigations</em> quotes the playwright Johann Nestroy and is found in three documents. Each result names its document and, if the remark was published, where.\n\nClick <strong>PI</strong> to read the remark in the <em>Investigations</em>.",
    },
    {
      page: HOME,
      target: `a.cover[href="${PI}"]`,
      click: `a.cover[href="${PI}"]`,
      skip: () => !!searchInput(),
      fallback: true,
      title: "Edited Writings",
      text: "Click <em>Philosophische Untersuchungen</em> (<em>Philosophical Investigations</em>) to open it.",
    },
    {
      page: PI,
      target: MOTTO_SECTION,
      click: `[id="${MOTTO}"] + .fac a`,
      title: "Remarks",
      text: "<strong>Ts-227a</strong> is the typescript this remark comes from, and <strong>1[2]</strong> links to a facsimile of its page. Tip: hover over a remark to reveal <strong>§</strong>; click it to copy a link to the remark you can share.\n\nClick <strong>1[2]</strong> to see the page.",
    },
    {
      page: PI,
      cue: ".lightbox:target .lightbox-close",
      done: () => !document.querySelector(".lightbox:target"),
      next: () => (location.hash = "_"),
      undo: () => (location.hash = "_"),
      title: "Facsimiles",
      text: "The typescript first had a different motto, a quote from Heinrich Hertz. Wittgenstein crossed it out and wrote the Nestroy quote above it by hand. Click the image to zoom in, and use ‹ and › to turn the pages.\n\nClose the page with × to continue.",
    },
    {
      page: PI,
      target: MOTTO_SECTION,
      cue: () => (document.querySelector(`${VERSIONS} details`)?.open ? TO_MS134 : `${VERSIONS} summary`),
      click: TO_MS134,
      next: () => (document.querySelector(`${VERSIONS} details`).open = true),
      undo: () => document.querySelector(`[id="${MOTTO}"] + .fac a`)?.click(),
      title: "Versions",
      text: "This remark also appears in other documents. <strong>←227b</strong> is an earlier version in Ts-227b; → marks later versions and ≈ versions from the same time. Useful for finding early drafts or later uses. <strong>+1</strong> means there is one more.\n\nOpen <strong>+1</strong> and click <strong>→134</strong>.",
    },
    {
      page: MS134,
      target: `.sticky-title, section:has(> [id="152-3"])`,
      click: ".sticky-title-text",
      boxes: ".sticky-title",
      fixed: true,
      title: "Title Bar",
      text: "The same motto appears in one of Wittgenstein’s notebooks, where he names its source: Nestroy’s play <em>Der Schützling</em>. It is dated 25 April 1947, after the typescript of the <em>Investigations</em> was made, so he must have added it there by hand later.\n\nWhen you follow a link to a remark, the bar at the top shows where you are. Click <strong>Ms-134</strong> in it to go to the top.",
    },
    {
      page: MS134,
      // The page header: title, downloads, and the overview lines below them.
      target: "main > h1, main > .doc-description, main > .lang-switcher, main > details > summary",
      cue: `${SIMILAR} > summary > span`,
      done: () => document.querySelector(SIMILAR)?.open,
      next: () => (document.querySelector(SIMILAR).open = true),
      title: "Overviews",
      text: "Every document and edited writing starts with an overview. Many of the remarks in this notebook reappear, reworked, in later typescripts.\n\nClick <strong>189 similar remarks</strong> to see where.",
    },
    {
      page: MS134,
      target: SIMILAR,
      cue: () => (document.querySelector(TS229)?.open ? `${TS229} > summary a[href="/ts-229/"]` : `${TS229} > summary > span`),
      click: `${TS229} > summary a[href="/ts-229/"]`,
      next: () => (document.querySelector(TS229).open = true),
      undo: () => (document.querySelector(SIMILAR).open = false),
      continues: true,
      title: "Overviews",
      text: "Almost all of them reappear in Ts-245 and in Ts-229. The visualization for Ts-229 connects each remark in the notebook (left) to the similar one in the typescript (right): remarks from all over the notebook were gathered into a few dozen pages.\n\nOpen <strong>184 similar</strong>, then click <strong>Ts-229</strong>.",
    },
    {
      page: "/ts-229/",
      target: "body > nav, main > h1, main > .doc-description, main > .lang-switcher",
      click: `body > nav a[href="${HOME}"]`,
      title: "Downloads",
      text: "Every document and edited writing can be downloaded as an epub or a pdf, and every document also as a pdf of its facsimiles. The title at the top of every page takes you back to the home page.\n\nClick <strong>Wittgenstein’s Writings</strong> to go back.",
    },
    {
      page: HOME,
      // The Nachlass Map's section: heading, note and preview (partials/map-home.html).
      target: "main > #nachlass-map, main > #nachlass-map + .section-note, main > .nachlass-map",
      click: "main > .nachlass-map a.cover",
      title: "Nachlass Map",
      text: "The Nachlass Map shows every remark of the Nachlass as a block, in the order they were written.\n\nClick the map to open it.",
    },
    {
      page: MAP,
      target: ".map-page .search-input",
      cue: ".map-page .search-input",
      skip: () => !searchInput(),
      done: () => /^nestr/i.test(searchInput()?.value.trim() ?? "") && document.querySelector(".map-page > .map .hit"),
      next: () => {
        const input = searchInput();
        input.value = "Nestroy";
        input.dispatchEvent(new Event("input", { bubbles: true }));
      },
      continues: true,
      title: "Nachlass Map",
      text: "The map has a search of its own, which marks its hits on the map.\n\nSearch for <strong>Nestroy</strong> again.",
    },
    {
      page: MAP,
      // The motto's hits and their documents' numbers (blocks have no content of their
      // own, so they count with their box).
      target: () => {
        const docs = MOTTO_DOCS.map((d) => document.querySelector(`.map-page > .map .d[data-doc="${d}"]`));
        return docs.filter(Boolean).flatMap((d) => [d.querySelector(".l"), ...d.querySelectorAll(".hit")]);
      },
      skip: () => !searchInput(),
      undo: clearSearch,
      continues: true,
      title: "Nachlass Map",
      text: "The map reads like a page of text, from 1914 at the top left to 1951 at the bottom: each number is a manuscript or typescript, each block one of its remarks. The motto is marked in the two typescripts of the <em>Investigations</em> and in a notebook from 1947.",
    },
    {
      page: MAP,
      target: ".map-page > .map-views",
      click: `.map-page > .map-views a[href="${MATH}"]`,
      title: "Themes",
      text: "Besides a search, the map can mark where a theme was worked on, all published remarks, or the remarks of one edited writing.\n\nClick <strong>Mathematics</strong>.",
    },
    {
      // Also on the Search view's page: the view switches in place once loaded.
      page: [MATH, MAP],
      target: '.map-page > .map .d[data-doc="Ms-122"]',
      boxes: ".d",
      continues: true,
      title: "Themes",
      text: "Here a block is darker the more its remark, and the remarks around it, are about mathematics. Ms-122 (<strong>122</strong>), a notebook from 1939–40, is almost all mathematics. Hovering over a block shows its page and the terms that mark it; clicking it opens the remark.\n\nClick <strong>Next</strong> to finish the tour.",
    },
    {
      // Back up to the page's title, the way home.
      page: [MATH, MAP],
      final: true,
      target: ".map-page > nav",
      title: "That’s the Tour!",
      text: "Click <strong>Wittgenstein’s Writings</strong> to go back to the home page, where you can take the tour again at any time.",
    },
  ];

  // --- State ------------------------------------------------------------------

  // sessionStorage, as JSON; null removes the key.
  const session = {
    get: (key) => {
      try {
        return JSON.parse(sessionStorage.getItem(key));
      } catch {
        return null;
      }
    },
    set: (key, v) => {
      try {
        v === null ? sessionStorage.removeItem(key) : sessionStorage.setItem(key, JSON.stringify(v));
      } catch {}
    },
  };
  const store = {
    get: () => session.get(KEY),
    set: (i) => {
      session.set(KEY, i);
      if (i === null) session.set(URLS, null);
    },
  };

  // The start button is hidden without JS and while a tour is running.
  const showStart = (on) =>
    document.querySelectorAll("[data-tour-start]").forEach((el) => (el.hidden = !on));

  const params = new URLSearchParams(location.search);
  if (params.has("tour")) {
    store.set(0);
    params.delete("tour");
    const q = params.toString();
    history.replaceState(null, "", location.pathname + (q ? `?${q}` : "") + location.hash);
  }
  if (store.get() === null) return showStart(true);

  // --- Elements ---------------------------------------------------------------

  const spot = document.createElement("div");
  spot.className = "tour-spot";
  const pop = document.createElement("div");
  pop.className = "tour";
  pop.setAttribute("role", "dialog");
  pop.setAttribute("aria-live", "polite");
  pop.innerHTML = `<button type="button" class="tour-close" aria-label="End tour">&times;</button>
    <h2 class="tour-title"></h2><div class="tour-text"></div>
    <p class="tour-actions"><span class="tour-count"></span><button type="button" class="tour-button tour-next"></button></p>`;
  const $ = (sel) => pop.querySelector(sel);

  let current = null; // index of the step shown on this page, or null
  let placed = null; // where the current step's highlight was when last scrolled into view
  let arriving = false; // the step was just shown on a page that's still loading
  let animating;

  const isCounted = (s) => !s.fallback && !s.final && !s.continues;
  const counted = steps.filter(isCounted);
  const fill = (i, title, text, next) => {
    const n = steps.slice(0, i).filter(isCounted).length + (isCounted(steps[i]) ? 1 : 0);
    $(".tour-count").textContent = steps[i].final ? "" : `${n} of ${counted.length}`;
    $(".tour-title").textContent = title;
    $(".tour-text").innerHTML = text
      .split(/\n\n/)
      .map((p) => `<p>${p}</p>`)
      .join("");
    $(".tour-next").textContent = next;
  };

  const cueOf = (step) => (typeof step.cue === "function" ? step.cue() : (step.cue ?? step.click));
  const setCue = (sel) => {
    const want = new Set(sel ? document.querySelectorAll(sel) : []);
    document.querySelectorAll(".tour-cue").forEach((el) => want.has(el) || el.classList.remove("tour-cue"));
    want.forEach((el) => el.classList.add("tour-cue"));
  };

  const pin = (on) => document.querySelector(".sticky-title")?.classList.toggle("tour-pinned", on);

  const hide = () => {
    current = null;
    setCue(null);
    pin(false);
    spot.remove();
    pop.remove();
  };
  const end = () => {
    store.set(null);
    hide();
    showStart(true);
  };

  // --- Layout -----------------------------------------------------------------

  const visible = (el) => (el.checkVisibility ? el.checkVisibility() : el.getClientRects().length > 0);

  // The boxes of an element's actual content: its lines of text and its images,
  // inputs etc., but not the margins and padding around them (a remark's text
  // column, for one, includes its last paragraph's bottom margin). Elements without
  // any such content count with their whole box.
  const REPLACED = "img, svg, input, textarea, select, video, canvas, iframe";
  const contentRects = (el) => {
    const rects = [];
    const walk = (node) => {
      if (node.nodeType === Node.TEXT_NODE) {
        if (!node.data.trim()) return;
        const range = document.createRange();
        range.selectNodeContents(node);
        rects.push(...range.getClientRects());
      } else if (node.nodeType === Node.ELEMENT_NODE && visible(node)) {
        if (node.matches(REPLACED)) rects.push(node.getBoundingClientRect());
        else node.childNodes.forEach(walk);
      }
    };
    walk(el);
    return rects.length ? rects : visible(el) ? [el.getBoundingClientRect()] : [];
  };

  // An element's whole box, extended by its absolutely positioned ::after, if any
  // (the sticky title bar draws its dividing line, wider than the bar, that way).
  const boxRects = (el) => {
    if (!visible(el)) return [];
    const box = el.getBoundingClientRect();
    const after = getComputedStyle(el, "::after");
    if (after.content === "none" || after.position !== "absolute") return [box];
    // ::after is positioned against the padding box (inside the borders).
    const style = getComputedStyle(el);
    const pad = {
      left: box.left + parseFloat(style.borderLeftWidth),
      right: box.right - parseFloat(style.borderRightWidth),
      bottom: box.bottom - parseFloat(style.borderBottomWidth),
    };
    const left = pad.left + parseFloat(after.left);
    const right = pad.right - parseFloat(after.right);
    const bottom = pad.bottom - parseFloat(after.bottom);
    const height = parseFloat(after.height);
    return [box, new DOMRect(left, bottom - height, right - left, height)];
  };

  // Union of the targets' boxes in page coordinates: content boxes, or whole boxes
  // for the elements matching `boxes`. It also covers the boxes of cued elements
  // inside a target, so that the cue's outline (6px around the element, like the
  // highlight) ends exactly at the highlight's edge.
  const rectOf = (sel, boxes) => {
    if (!sel) return null;
    const targets = typeof sel === "function" ? sel() : [...document.querySelectorAll(sel)];
    const rects = targets.flatMap((el) => (boxes && el.matches(boxes) ? boxRects(el) : contentRects(el)));
    for (const el of document.querySelectorAll(".tour-cue")) {
      if (visible(el) && targets.some((t) => t.contains(el))) rects.push(el.getBoundingClientRect());
    }
    let t = Infinity, l = Infinity, b = -Infinity, r = -Infinity;
    for (const x of rects) {
      if (!x.width && !x.height) continue;
      t = Math.min(t, x.top), l = Math.min(l, x.left), b = Math.max(b, x.bottom), r = Math.max(r, x.right);
    }
    if (t === Infinity) return null;
    return { top: t + scrollY, left: l + scrollX, bottom: b + scrollY, right: r + scrollX };
  };

  // Scroll smoothly, except right after arriving on a page: the browser's jump to
  // a #fragment can interrupt a smooth scroll while the page loads, but not undo an
  // instant one.
  const scrollToY = (y) => scrollTo({ top: Math.max(0, y), behavior: arriving ? "instant" : "smooth" });

  const px = (el, props) => {
    for (const [k, v] of Object.entries(props)) el.style[k] = `${Math.round(v)}px`;
  };

  // Space taken at the top of the screen by the title bar (work/doc pages, while a
  // remark is the #fragment, see style.css).
  const topInset = () => Math.max(0, document.querySelector(".sticky-title")?.getBoundingClientRect().bottom ?? 0);

  // Position spot and popover for the current step: the popover right below the
  // highlight. With `scroll`, also scroll both into view: centered if they fit on
  // the screen, otherwise the popover at the bottom with as much of the highlight
  // as fits above it, as long as some of the cued element is in view. Without
  // `scroll`, they are still scrolled into view if the highlight or the popover
  // moved or changed size since (e.g. web fonts or images loaded, or a <details>
  // opened).
  const layout = (scroll) => {
    const step = steps[current];
    const vw = document.documentElement.clientWidth, vh = innerHeight;
    const r = rectOf(step.target, step.boxes);
    const w = pop.offsetWidth, h = pop.offsetHeight;

    spot.classList.toggle("tour-spot-off", !r);
    if (!r) {
      // Untargeted: at the bottom of the screen (re-run on scroll, see below).
      return px(pop, { top: scrollY + vh - h - MARGIN, left: scrollX + (vw - w) / 2 });
    }
    px(spot, {
      top: r.top - PAD,
      left: r.left - PAD,
      width: r.right - r.left + 2 * PAD,
      height: r.bottom - r.top + 2 * PAD,
    });
    const target = { top: r.top - PAD, bottom: r.bottom + PAD };
    const top = target.bottom + GAP;
    px(pop, { top, left: Math.max(MARGIN, Math.min(r.left - PAD, vw - w - MARGIN)) });

    // (The top of a fixed target, the bar, moves in page coordinates as the page
    // scrolls, so only its bottom counts.)
    const where = `${step.fixed ? "" : target.top},${target.bottom},${h}`;
    if (!scroll && where === placed) return;
    placed = where;
    if (step.fixed) {
      // The bar stays at the top of the screen and the target's other part
      // (the remark) just below it: only scroll down, as little as needed.
      const below = top + h - (scrollY + vh - MARGIN);
      if (below > 0) scrollToY(scrollY + below);
      return;
    }

    const block = { top: target.top, bottom: top + h };
    const cue = [...document.querySelectorAll(".tour-cue")].find(visible)?.getBoundingClientRect();
    const scrollFor = (inset) => {
      const view = { top: inset + MARGIN, bottom: vh - MARGIN };
      const room = view.bottom - view.top;
      if (block.bottom - block.top <= room) return block.top - view.top - (room - (block.bottom - block.top)) / 2;
      const y = block.bottom - view.bottom;
      // Unless that leaves (almost) nothing of the cued element in view: then
      // show it from its top, even if the popover doesn't fit below anymore.
      return cue && cue.bottom + scrollY < y + view.top + 2 * MARGIN ? cue.top + scrollY - PAD - view.top : y;
    };
    const inset = topInset();
    if (block.top >= scrollY + inset + MARGIN && block.bottom <= scrollY + vh - MARGIN) return;
    scrollToY(Math.max(0, scrollFor(inset)));
  };

  // Let the next position change of both boxes transition.
  const animate = () => {
    spot.classList.add("tour-animate");
    pop.classList.add("tour-animate");
    clearTimeout(animating);
    animating = setTimeout(() => {
      spot.classList.remove("tour-animate");
      pop.classList.remove("tour-animate");
    }, ANIMATION + 50);
  };

  // --- Steps ------------------------------------------------------------------

  // Checked before a click navigates away, so a skip condition is always evaluated
  // on the page the skipped step belongs to.
  const unskipped = (i) => {
    while (steps[i]?.skip?.()) i++;
    return i;
  };

  // Whether a step lives on this page, and the page to load to go back to it.
  const onPage = (step) => [].concat(step.page).includes(location.pathname);
  const pageOf = (step) => [].concat(step.page)[0];

  const go = (i) => {
    i = unskipped(i);
    if (i >= steps.length) return end();
    store.set(i);
    // A step on another page: the click that got us here is navigating there.
    if (!onPage(steps[i])) return hide();
    show(i);
  };

  const show = (i) => {
    const step = steps[i];
    if (step.done?.()) return go(i + 1);
    const moving = pop.isConnected; // already showing a step on this page: animate
    current = i;
    placed = null;
    fill(i, step.title, step.text, step.final ? "Finish" : "Next");
    $(".tour-next").onclick = () => {
      const el = step.click && document.querySelector(step.click);
      if (el && visible(el)) el.click();
      else if (step.next) step.next();
      else go(i + 1);
    };
    setCue(cueOf(step));
    pin(!!step.fixed);
    showStart(!!step.final); // the closing message points at it
    arriving = !moving && document.readyState !== "complete";
    if (moving) animate();
    else {
      document.body.append(spot, pop);
      // On a reload or going back, don't let the browser restore the previous
      // scroll position after we've scrolled to the step.
      history.scrollRestoration = "manual";
    }
    layout(true);
    // Once the page has loaded, again: by then the browser may have jumped to the
    // #fragment (again, once web fonts and images have shifted the layout).
    if (document.readyState !== "complete") {
      addEventListener(
        "load",
        () =>
          requestAnimationFrame(() => {
            if (current === i) layout(true);
            arriving = false;
          }),
        { once: true },
      );
    }
  };

  // Go back to step i, or the closest step before it that makes sense to show on
  // this page: not one that is skipped or done already (it would advance again right
  // away), nor one whose preceding step on this page isn't done yet (after a reload,
  // say, a <details> it needs open is closed again). A step on another page is gone
  // back to by loading that page at the URL it was left at, and settled there.
  const backTo = (i) => {
    const here = (j) => !!steps[j] && onPage(steps[j]);
    while (here(i) && (steps[i].skip?.() || steps[i].done?.())) i--;
    while (here(i - 1) && steps[i - 1].done && !steps[i - 1].done()) i--;
    if (i < 0) return;
    store.set(i);
    if (here(i)) return show(i);
    session.set(BACK, true);
    location.href = session.get(URLS)?.[pageOf(steps[i])] ?? pageOf(steps[i]);
  };

  const back = () => {
    const i = current;
    if (i === null || i === 0) return;
    // Paused until we're back, so that the changes the undo makes don't advance the
    // step (e.g. closing the lightbox finishes the facsimile step).
    current = null;
    steps[i].undo?.();
    // After the undo has taken effect (a hash change, a toggled <details>).
    setTimeout(() => backTo(i - 1));
  };

  // --- Events -----------------------------------------------------------------

  $(".tour-close").onclick = end;

  // Enter and → for "Next", ← to go back. Not while typing in a field, where the
  // arrows move the cursor (Enter in the search box does count), and Enter not on a
  // link or button, which it activates.
  document.addEventListener("keydown", (e) => {
    if (current === null || e.defaultPrevented || e.isComposing || e.repeat) return;
    if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    const field = e.target.closest?.("input, textarea, select, [contenteditable]");
    if (e.key === "Enter") {
      if (e.target.closest?.("a, button, summary, textarea, select, [contenteditable]")) return;
      $(".tour-next").click();
    } else if (e.key === "ArrowRight" && !field) $(".tour-next").click();
    else if (e.key === "ArrowLeft" && !field) back();
    else return;
    e.preventDefault();
  });

  // Remember the URL each page is left at (with the search query or the #fragment),
  // to go back to it.
  addEventListener("pagehide", () => {
    if (store.get() === null) return;
    session.set(URLS, { ...session.get(URLS), [location.pathname]: location.href });
  });

  // Capture phase, so we see the click before the link navigates. Save progress
  // right away (the page may be about to unload), but show the next step only after
  // the click's default action (e.g. opening a lightbox via its #hash) has happened.
  document.addEventListener(
    "click",
    (e) => {
      const step = steps[current];
      if (!step?.click || !e.target.closest?.(step.click)) return;
      const next = unskipped(current + 1);
      store.set(next);
      setTimeout(() => go(next));
    },
    true,
  );

  // Something changed (a <details> toggled, a lightbox opened or closed, search
  // results arrived, the layout moved): advance if the step is done, otherwise
  // follow the target without scrolling and re-mark the cue (e.g. new results).
  const update = () => {
    if (current === null) return;
    const step = steps[current];
    if (step.done?.()) return go(current + 1);
    setCue(cueOf(step));
    layout(false);
  };
  document.addEventListener("toggle", update, true); // doesn't bubble, but can be captured
  document.addEventListener("input", update, true); // e.g. typing more of the search term
  addEventListener("hashchange", update);
  addEventListener("resize", update);
  new ResizeObserver(update).observe(document.body);
  new MutationObserver((records) => {
    // Ignore our own changes (appending the boxes), which would otherwise loop.
    if (records.some((r) => ![...r.addedNodes, ...r.removedNodes].every((n) => n === spot || n === pop))) update();
  }).observe(document.body, { childList: true, subtree: true });
  // Targeted steps scroll with the page; untargeted and fixed ones stay on screen.
  addEventListener(
    "scroll",
    () => current !== null && (!steps[current].target || steps[current].fixed) && layout(false),
    { passive: true },
  );

  // On another page than the current step's, the tour is paused: show nothing but
  // the start button (on the homepage), to start over.
  // Going back to a step on this page, settle on one that makes sense here.
  const i = store.get();
  const goingBack = session.get(BACK);
  session.set(BACK, null);
  if (!steps[i]) end();
  else if (!onPage(steps[i])) showStart(true);
  else if (goingBack) backTo(i);
  else go(i);
})();
