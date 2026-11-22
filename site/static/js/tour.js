// Guided tour of the site: a popover that walks through the main features step by
// step, highlighting the element to look at, marking the one to click, and advancing
// once the reader has done so. Self-contained and inert unless a tour is in progress:
// it is started by the callout on the homepage (partials/intro.html, a link to
// "/?tour"), and the current step is kept in sessionStorage so the tour survives
// navigating between pages (and is forgotten when the tab is closed). On a page
// other than the current step's, the tour stays out of the way: it pauses, and
// picks up again on returning to that page (e.g. with the back button).
//
// The highlight and the popover are positioned in page coordinates, so they scroll
// with the content. Moving to a step on the same page animates both boxes (CSS
// transitions) while a single smooth scroll brings them into view.
(() => {
  const KEY = "tour-step";
  const ANIMATION = 400; // ms, matches the transitions in style.css
  const NARROW = 640; // px; below this the popover is a bottom sheet (style.css)
  const PAD = 6, GAP = 12, MARGIN = 16;

  // --- Steps ------------------------------------------------------------------
  // page: pathname the step lives on. target: element(s) to highlight (the highlight
  // spans the content of all matches, e.g. a remark's margin columns and text).
  // click: advance when this is clicked ("Next" clicks it, if visible). cue:
  // element(s) to mark as the thing to interact with (default: click); a function
  // is re-evaluated whenever the page changes. done: advance once this is true
  // (checked whenever the page changes). next: what "Next" does otherwise.
  // skip: leave the step out when this is true (checked on its own page).
  // fallback: not counted in "n of m" (it stands in for skipped steps).
  // final: the closing message: not counted, no "n of m", and "Finish"; the start
  // callout is shown again for it.
  // boxes: target elements measured by their whole box (and ::after) rather than
  // their content, e.g. the sticky title bar with its dividing line.
  // fixed: the target includes a fixed element (the sticky title bar): don't scroll,
  // and follow the target while the page scrolls.
  // text: paragraphs separated by blank lines.
  //
  // The route follows the motto of the Philosophical Investigations, which quotes
  // Nestroy: search → PI → its facsimile → a later version in Ms-134 → Ms-134's
  // similar remarks → Ts-229, where many of them were reworked.

  const PI = "/w-pi/";
  const MOTTO = "ts-227a-1-2"; // the motto's id in PI
  const MOTTO_SECTION = `section:has(> [id="${MOTTO}"])`;
  const VERSIONS = `[id="${MOTTO}"] .versions`;
  const TO_MS134 = `${VERSIONS} a[href="/ms-134/#1523"]`;
  const RESULT_LINK = `a[href="${PI}#${MOTTO}"]`; // the search result's "PI" link
  const RESULT = `.search-results ${RESULT_LINK}`;
  const MS134 = "/ms-134/";
  const SIMILAR = "main > details.similar";
  const TS229 = `${SIMILAR} > details:has(> summary a[href="/ts-229/"])`;
  const searchInput = () => document.querySelector(".search:not([hidden]) .search-input");

  const steps = [
    {
      page: "/",
      target:
        'main > h2:first-of-type, main > h2:first-of-type + .section-note, a.cover[href^="/w-"]:nth-of-type(-n+3)',
      title: "Works and documents",
      text: "These are the works compiled by Wittgenstein’s literary executors. Further down are his individual manuscripts and typescripts, which you can also list by name or date.",
    },
    {
      page: "/",
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
      text: "You can also search all transcriptions at once. While you search, the works and documents are hidden; clear the search to bring them back.\n\nTry it: search for <strong>Nestroy</strong>.",
    },
    {
      page: "/",
      target: `.search-result:has(${RESULT_LINK})`,
      click: RESULT,
      skip: () => !searchInput(),
      title: "Search results",
      text: "The motto of the <em>Philosophical Investigations</em> quotes the playwright Johann Nestroy and is found in three documents. Each result names its document and, if the remark was published, the work it appears in.\n\nClick <strong>PI</strong> to read the remark in the <em>Investigations</em>.",
    },
    {
      page: "/",
      target: `a.cover[href="${PI}"]`,
      click: `a.cover[href="${PI}"]`,
      skip: () => !!searchInput(),
      fallback: true,
      title: "Philosophical Investigations",
      text: "Click <em>Philosophical Investigations</em> to open it.",
    },
    {
      page: PI,
      target: MOTTO_SECTION,
      click: `[id="${MOTTO}"] + .fac a`,
      title: "Remarks",
      text: "<strong>Ts-227a</strong> is the typescript this remark comes from, and <strong>1[2]</strong> links to a facsimile of its page. Tip: hover over a remark to reveal <strong>§</strong>, a link you can share.\n\nClick <strong>1[2]</strong> to see the page.",
    },
    {
      page: PI,
      cue: ".lightbox:target .lightbox-close",
      done: () => !document.querySelector(".lightbox:target"),
      next: () => (location.hash = "_"),
      title: "Facsimile",
      text: "The typescript first had a different motto, a quote from Heinrich Hertz. Wittgenstein crossed it out and wrote the Nestroy quote above it by hand. Click the image to zoom in, and use ‹ and › to turn the pages.\n\nClose the page with × to continue.",
    },
    {
      page: PI,
      target: MOTTO_SECTION,
      cue: () => (document.querySelector(`${VERSIONS} details`)?.open ? TO_MS134 : `${VERSIONS} summary`),
      click: TO_MS134,
      next: () => (document.querySelector(`${VERSIONS} details`).open = true),
      title: "Versions",
      text: "This remark also appears in other documents. <strong>←227b</strong> is an earlier version in Ts-227b; → marks later versions and ≈ versions from the same time. Useful for finding early drafts or later uses. <strong>+1</strong> means there is one more.\n\nOpen <strong>+1</strong> and click <strong>→134</strong>.",
    },
    {
      page: MS134,
      target: `.sticky-title, section:has(> [id="1523"])`,
      click: ".sticky-title-text",
      boxes: ".sticky-title",
      fixed: true,
      title: "Ms-134",
      text: "The same motto appears in one of Wittgenstein’s notebooks, where he names its source: Nestroy’s play <em>Der Schützling</em>. It is dated 25 April 1947, after the typescript of the <em>Investigations</em> was made, so he must have added it there by hand later.\n\nThe bar at the top shows where you are. Click <strong>Ms-134</strong> in it to go to the top.",
    },
    {
      page: MS134,
      // The page header: title, downloads, and the overview lines below them.
      target: "main > h1, main > .lang-switcher, main > details > summary",
      cue: `${SIMILAR} > summary > span`,
      done: () => document.querySelector(SIMILAR)?.open,
      next: () => (document.querySelector(SIMILAR).open = true),
      title: "Overview",
      text: "Every work and document starts with an overview. Many of the remarks in this notebook reappear, reworked, in later typescripts.\n\nClick <strong>189 similar remarks</strong> to see where.",
    },
    {
      page: MS134,
      target: SIMILAR,
      cue: () => (document.querySelector(TS229)?.open ? `${TS229} > summary a[href="/ts-229/"]` : `${TS229} > summary > span`),
      click: `${TS229} > summary a[href="/ts-229/"]`,
      next: () => (document.querySelector(TS229).open = true),
      title: "Similar remarks",
      text: "Almost all of them reappear in Ts-245 and in Ts-229. The visualization for Ts-229 connects each remark in the notebook (left) to the similar one in the typescript (right): remarks from all over the notebook were gathered into a few dozen pages.\n\nOpen <strong>184 similar</strong>, then click <strong>Ts-229</strong>.",
    },
    {
      page: "/ts-229/",
      target: "body > nav, main > h1, main > .lang-switcher",
      click: 'body > nav a[href="/"]',
      title: "Downloads",
      text: "Every work and document can be downloaded as an epub or a pdf, and every document also as a pdf of its facsimiles. The title at the top of every page takes you back home.\n\nClick <strong>Wittgenstein’s Writings</strong> to go home.",
    },
    {
      page: "/",
      final: true,
      target: ".tour-callout",
      boxes: ".tour-callout",
      title: "That’s the tour!",
      text: "You’re back at the start. You can take the tour again at any time with this button.",
    },
  ];

  // --- State ------------------------------------------------------------------

  const store = {
    get: () => {
      try {
        const v = sessionStorage.getItem(KEY);
        return v === null ? null : Number(v);
      } catch {
        return null;
      }
    },
    set: (i) => {
      try {
        i === null ? sessionStorage.removeItem(KEY) : sessionStorage.setItem(KEY, i);
      } catch {}
    },
  };

  // The start callout is hidden without JS and while a tour is running.
  const showCallout = (on) =>
    document.querySelectorAll("[data-tour-start]").forEach((el) => (el.hidden = !on));

  const params = new URLSearchParams(location.search);
  if (params.has("tour")) {
    store.set(0);
    params.delete("tour");
    const q = params.toString();
    history.replaceState(null, "", location.pathname + (q ? `?${q}` : "") + location.hash);
  }
  if (store.get() === null) return showCallout(true);

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
  let side = null; // where the popover goes for the current step: below, above or over the target
  let animating;

  const isCounted = (s) => !s.fallback && !s.final;
  const counted = steps.filter(isCounted);
  const fill = (i, title, text, next) => {
    const n = steps.slice(0, i).filter(isCounted).length + (steps[i].fallback ? 0 : 1);
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

  const hide = () => {
    current = null;
    setCue(null);
    spot.remove();
    pop.remove();
  };
  const end = () => {
    store.set(null);
    hide();
    showCallout(true);
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
    const targets = [...document.querySelectorAll(sel)];
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

  const px = (el, props) => {
    for (const [k, v] of Object.entries(props)) el.style[k] = `${Math.round(v)}px`;
  };

  // Space taken at the top of the screen by the sticky title bar (work/doc pages).
  const topInset = () => document.querySelector(".sticky-title")?.offsetHeight ?? 0;

  // Position spot and popover for the current step. With `scroll`, also decide
  // where the popover goes and scroll both into view.
  const layout = (scroll) => {
    const step = steps[current];
    const vw = document.documentElement.clientWidth, vh = innerHeight;
    const narrow = vw < NARROW;
    const r = rectOf(step.target, step.boxes);
    const w = pop.offsetWidth, h = pop.offsetHeight;

    spot.classList.toggle("tour-spot-off", !r);
    if (r) {
      px(spot, {
        top: r.top - PAD,
        left: r.left - PAD,
        width: r.right - r.left + 2 * PAD,
        height: r.bottom - r.top + 2 * PAD,
      });
    }

    if (!r) {
      // Untargeted: at the bottom of the screen (re-run on scroll, see below).
      if (narrow) return pop.removeAttribute("style");
      return px(pop, { top: scrollY + vh - h - MARGIN, left: scrollX + (vw - w) / 2 });
    }

    const view = { top: (step.fixed ? 0 : topInset()) + MARGIN, bottom: vh - MARGIN };
    const target = { top: r.top - PAD, bottom: r.bottom + PAD };
    if (narrow) {
      pop.removeAttribute("style");
      view.bottom = vh - h - 2 * MARGIN; // above the bottom sheet
    }
    const room = view.bottom - view.top;

    // Place the popover below the target, above it (near the end of the page), or,
    // for targets too tall to show together with the popover, over the target's
    // lower part with the target's top in view. If the target changes size so that
    // the placement changes (e.g. a <details> inside it opens), move there smoothly.
    const docH = document.documentElement.scrollHeight;
    const size = target.bottom - target.top + (narrow ? 0 : GAP + h);
    const fits = !narrow && size > room ? "over" : narrow || target.bottom + GAP + h + MARGIN <= docH ? "below" : "above";
    if (side && side !== fits) {
      animate();
      scroll = true;
    }
    side = fits;

    let block = target;
    if (!narrow) {
      let top;
      if (side === "below") top = target.bottom + GAP;
      else if (side === "above") top = target.top - GAP - h;
      else top = target.top + room - h;
      const left = Math.max(MARGIN, Math.min(r.left - PAD, vw - w - MARGIN));
      px(pop, { top, left });
      block = { top: Math.min(target.top, top), bottom: Math.max(target.bottom, top + h) };
    }

    if (!scroll || step.fixed) return;
    const inView = block.top >= scrollY + view.top && block.bottom <= scrollY + view.bottom;
    if (inView) return;
    const blockSize = block.bottom - block.top;
    // Center the block if it fits; otherwise show its top.
    const y = blockSize <= room ? block.top - view.top - (room - blockSize) / 2 : block.top - view.top;
    scrollTo({ top: Math.max(0, y), behavior: "smooth" });
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

  const go = (i) => {
    i = unskipped(i);
    if (i >= steps.length) return end();
    store.set(i);
    // A step on another page: the click that got us here is navigating there.
    if (steps[i].page !== location.pathname) return hide();
    show(i);
  };

  const show = (i) => {
    const step = steps[i];
    if (step.done?.()) return go(i + 1);
    const moving = pop.isConnected; // already showing a step on this page: animate
    current = i;
    side = null;
    fill(i, step.title, step.text, step.final ? "Finish" : "Next");
    $(".tour-next").onclick = () => {
      const el = step.click && document.querySelector(step.click);
      if (el && visible(el)) el.click();
      else if (step.next) step.next();
      else go(i + 1);
    };
    setCue(cueOf(step));
    showCallout(!!step.final); // the closing message points at it
    if (moving) animate();
    else document.body.append(spot, pop);
    layout(true);
  };

  // --- Events -----------------------------------------------------------------

  $(".tour-close").onclick = end;

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
  // the start callout (on the homepage), to start over.
  const i = store.get();
  if (!steps[i]) end();
  else if (steps[i].page === location.pathname) go(i);
  else showCallout(true);
})();
