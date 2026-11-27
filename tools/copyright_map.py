#!/usr/bin/env python3
"""Draw the About page's world map of default copyright terms, legend included.

Usage: tools/copyright_map.py [countries-110m.json] [countries.json] > site/static/images/copyright-map.svg

The map shows each country's default copyright term (the author's life + n years), to give an
idea of how much the terms vary; it does not show where Wittgenstein's Nachlass is in the public
domain, which depends on the history of each item (see the About page's Copyright section).

Terms: Wikimedia Commons' File:World_copyright_terms.svg, whose stylesheet lists the countries
by ISO 3166 alpha-2 code under each term (copied into TERMS below), except Guinea, whose 2019
law replaced life + 80 with life + 70. Countries the Commons map does not classify are drawn
blank, outlined, and left out of the legend.

Geometry: Natural Earth 1:110m countries (public domain), as TopoJSON from the world-atlas
package. Its countries are identified by ISO 3166 numeric code, which the world-countries
package maps to alpha-2. Both are downloaded from jsDelivr when no files are given.
Projection: Equal Earth.

Colors are the site's tokens (site/static/css/style.css): the terms are shades from
--color-highlight (shortest) to --color-text (longest), --color-bg for the borders and the unknown
countries, --color-margin for the outline of the unknown countries, with the dark-mode values
under prefers-color-scheme (which an SVG shown as <img> follows from its page). An SVG shown as
<img> cannot load the page's web fonts, so the legend asks for TeX Gyre Pagella and falls back
to Palatino, which it is a clone of.
"""

import json
import math
import sys
import urllib.request

WORLD_ATLAS = "https://cdn.jsdelivr.net/npm/world-atlas@2.0.2/countries-110m.json"
WORLD_COUNTRIES = "https://cdn.jsdelivr.net/npm/world-countries@5.1.0/countries.json"

# Copyright term (years after the author's death) -> ISO 3166 alpha-2 codes, from Commons.
TERMS = {
    25: "ly",
    30: "so",
    50: "ae af ag ai aw bi bn bo bt bw bz cd cf cg ck cm cn cu cv cw dj do dz eg er et fj fk fm"
    " ga gd gm gw gy iq ir jo ke kg kh ki km kp kw la lb lc lr ls mc mm mn ms mu mw my na np nr"
    " nu nz pg ph pk pw qa rw sa sb sc sd sh sl sm sr ss sy sz tc tg th tj tk tl tm tn to tt tv"
    " tz ug vg vn vu ye za zm zw",
    60: "bd ht in ve",
    70: "ad al am ao ar at au az ba bb be bf bg bh bj bl bm br bs by ca ch ci cl cr cy cz de dk"
    " dm ec ee es fi fo fr gb ge gg gh gi gl gr gu hn hr hu id ie il im is it je jp kn kr ky kz"
    " li lk lt lu lv ma md me mf mg mk ml mp mr mt mv mz nc ne nf ng ni nl no om pa pe pf pl pm"
    " pn pr pt py ro rs ru se sg si sk sn st sv sx td tr ua us uy uz va vi wf xk"
    " gn",  # Guinea: life + 80 on Commons, life + 70 since 2019
    75: "gt vc ws",
    80: "co gq",
    95: "jm",
    100: "mx",
}
TERM_OF = {code: term for term, codes in TERMS.items() for code in codes.split()}

# Countries of the 1:110m data without an ISO numeric code there.
CODE_BY_NAME = {"Kosovo": "xk"}

EXCLUDED = {"010"}  # Antarctica

# Site tokens: (light, dark).
HIGHLIGHT = ("#fbcba4", "#5f3a1d")
TEXT = ("#444444", "#cccccc")
BG = ("#ffffff", "#1e1e1e")
MARGIN = ("#bbbbbb", "#525252")

FONT = "'TeX Gyre Pagella','Palatino Linotype',Palatino,'URW Palladio L',serif"

# Central meridian: ~11°E, as on most world maps, so that the cut through the Pacific falls
# in open sea and the continents sit in the middle.
CENTER_LON = 11

WIDTH = 1000


def oklab(hex_color):
    def linear(c):
        c /= 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (linear(int(hex_color[i : i + 2], 16)) for i in (1, 3, 5))
    l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b) ** (1 / 3)
    m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b) ** (1 / 3)
    s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b) ** (1 / 3)
    return (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )


def srgb(lab):
    L, a, b = lab
    l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
    m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
    s = (L - 0.0894841775 * a - 1.2914855480 * b) ** 3
    rgb = (
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    )

    def gamma(c):
        c = min(max(c, 0), 1)
        return 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055

    return "#" + "".join(f"{round(gamma(c) * 255):02x}" for c in rgb)


def mix(c0, c1, t):
    """Mix two colors in OKLab, so that the shades step evenly in lightness."""
    return srgb(tuple(x + (y - x) * t for x, y in zip(oklab(c0), oklab(c1))))


def equal_earth(lon, lat):
    a1, a2, a3, a4 = 1.340264, -0.081106, 0.000893, 0.003796
    m = math.sqrt(3) / 2
    lam, phi = math.radians(lon), math.radians(lat)
    theta = math.asin(m * math.sin(phi))
    t2 = theta * theta
    t6 = t2 * t2 * t2
    x = lam * math.cos(theta) / (m * (a1 + 3 * a2 * t2 + t6 * (7 * a3 + 9 * a4 * t2)))
    y = theta * (a1 + a2 * t2 + t6 * (a3 + a4 * t2))
    return x, -y


def decode_arcs(topo):
    (sx, sy), (tx, ty) = topo["transform"]["scale"], topo["transform"]["translate"]
    arcs = []
    for arc in topo["arcs"]:
        x = y = 0
        points = []
        for dx, dy in arc:
            x, y = x + dx, y + dy
            points.append((x * sx + tx, y * sy + ty))
        arcs.append(points)
    return arcs


def ring_points(ring, arcs):
    points = []
    for i in ring:
        arc = arcs[i] if i >= 0 else arcs[~i][::-1]
        points.extend(arc[1:] if points else arc)
    return points


def clip(ring, keep):
    """Sutherland–Hodgman clip of a lon/lat ring against one half-plane, keep(lon) >= 0."""
    out = []
    for i, cur in enumerate(ring):
        prev = ring[i - 1]
        kc, kp = keep(cur[0]), keep(prev[0])
        if (kc >= 0) != (kp >= 0):
            t = kp / (kp - kc)
            out.append((prev[0] + t * (cur[0] - prev[0]), prev[1] + t * (cur[1] - prev[1])))
        if kc >= 0:
            out.append(cur)
    return out


def split_antimeridian(ring):
    """Rings crossing ±180° (Russia, Fiji) jump across the map: unwrap their longitudes,
    then cut them into the part inside [-180, 180] and the part beyond, shifted back."""
    ring = [((lon - CENTER_LON + 180) % 360 - 180, lat) for lon, lat in ring]
    unwrapped = [ring[0]]
    for lon, lat in ring[1:]:
        prev = unwrapped[-1][0]
        while lon - prev > 180:
            lon -= 360
        while lon - prev < -180:
            lon += 360
        unwrapped.append((lon, lat))
    if all(-180 <= lon <= 180 for lon, _ in unwrapped):
        return [ring]
    pieces = []
    for shift in (-360, 0, 360):
        piece = [(lon + shift, lat) for lon, lat in unwrapped]
        piece = clip(clip(piece, lambda lon: lon + 180), lambda lon: 180 - lon)
        if len(piece) >= 3:
            pieces.append(piece)
    return pieces


def polygons(geometry):
    if geometry["type"] == "Polygon":
        return [geometry["arcs"]]
    if geometry["type"] == "MultiPolygon":
        return geometry["arcs"]
    return []


def load(path, url):
    if path:
        with open(path) as f:
            return json.load(f)
    with urllib.request.urlopen(url) as r:
        return json.load(r)


def main():
    topo = load(sys.argv[1] if len(sys.argv) > 1 else None, WORLD_ATLAS)
    alpha2 = {
        c["ccn3"]: c["cca2"].lower()
        for c in load(sys.argv[2] if len(sys.argv) > 2 else None, WORLD_COUNTRIES)
        if c.get("ccn3")
    }
    terms = sorted(TERMS)

    arcs = decode_arcs(topo)
    countries = []  # (name, term or None, [projected rings])
    for g in topo["objects"]["countries"]["geometries"]:
        if g.get("id") in EXCLUDED:
            continue
        name = g["properties"]["name"]
        code = alpha2.get(g.get("id")) or CODE_BY_NAME.get(name)
        rings = [
            [equal_earth(lon, lat) for lon, lat in piece]
            for polygon in polygons(g)
            for ring in polygon
            for piece in split_antimeridian(ring_points(ring, arcs))
        ]
        countries.append((name, TERM_OF.get(code), rings))

    xs = [x for _, _, rings in countries for ring in rings for x, _ in ring]
    ys = [y for _, _, rings in countries for ring in rings for _, y in ring]
    pad = 0.01
    x0, x1, y0, y1 = min(xs) - pad, max(xs) + pad, min(ys) - pad, max(ys) + pad
    k = WIDTH / (x1 - x0)
    map_height = round((y1 - y0) * k)

    def path(rings):
        d = []
        for ring in rings:
            pts = []
            for x, y in ring:
                p = (round((x - x0) * k, 1), round((y - y0) * k, 1))
                if not pts or p != pts[-1]:
                    pts.append(p)
            if len(pts) < 3:
                continue
            d.append("M" + "L".join(f"{x:g},{y:g}" for x, y in pts) + "Z")
        return "".join(d)

    # Legend, below the map: a row of swatches with the terms under them, read as
    # "Author's life + 25 years".
    swatch_w, swatch_h, font_size = 46, 18, 22
    strip_x = 340
    swatch_y = map_height + 24
    label_y = swatch_y + swatch_h + font_size + 4
    height = label_y + 8

    def styles(mode):
        rules = [f"path{{fill:{BG[mode]};stroke:{BG[mode]}}}"]
        rules += [f".t{i}{{fill:{mix(HIGHLIGHT[mode], TEXT[mode], i / (len(terms) - 1))}}}"
                  for i in range(len(terms))]
        rules += [f"path.u{{stroke:{MARGIN[mode]}}}", f"text{{fill:{TEXT[mode]}}}"]
        return "".join(rules)

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {height}">',
        "<style>",
        "path{stroke-width:0.5;stroke-linejoin:round}",
        f"text{{font-family:{FONT};font-size:{font_size}px;text-anchor:middle}}",
        styles(0),
        f"@media (prefers-color-scheme:dark){{{styles(1)}}}",
        "</style>",
    ]
    # The unknown countries last, so that their outlines are not covered by their neighbors'.
    for name, term, rings in sorted(countries, key=lambda c: (c[1] is None, c[0])):
        cls = f"t{terms.index(term)}" if term else "u"
        title = f"{name}: life + {term} years" if term else f"{name}: unknown"
        out.append(f'<path class="{cls}" d="{path(rings)}"><title>{title}</title></path>')

    out.append(f'<text x="{strip_x - 12}" y="{label_y}" style="text-anchor:end">Author’s life +</text>')
    for i, term in enumerate(terms):
        x = strip_x + i * swatch_w
        out.append(f'<rect class="t{i}" x="{x}" y="{swatch_y}" width="{swatch_w}" height="{swatch_h}"/>')
        out.append(f'<text x="{x + swatch_w / 2:g}" y="{label_y}">{term}</text>')
    out.append(f'<text x="{strip_x + len(terms) * swatch_w + 12}" y="{label_y}" style="text-anchor:start">years</text>')
    out.append("</svg>")
    print("\n".join(out))


if __name__ == "__main__":
    main()
