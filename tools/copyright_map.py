#!/usr/bin/env python3
"""Draw the About page's world map of where Wittgenstein's Nachlass is in the public domain.

Usage: tools/copyright_map.py [countries-110m.json] > site/static/images/copyright-map.svg

Geometry: Natural Earth 1:110m countries (public domain), as TopoJSON from the world-atlas
package; downloaded from jsDelivr when no file is given. Projection: Equal Earth.

Wittgenstein died in 1951, so his writings are in the public domain wherever the copyright
term is the author's life + 70 years or less. The countries with longer terms (the
classification of Wikimedia Commons' File:World_copyright_terms.svg, minus Guinea, whose 2019
law replaced life + 80 with life + 70 for all works still protected) are listed below with
their terms; each of them is drawn as public domain once its term has run out, so re-running
the script in a later year updates the map.

Colors are the site's tokens (site/static/css/style.css): --color-highlight for the public
domain, --color-text for the rest, stripes of both for the partly public domain (the US),
--color-bg for the borders, with the dark-mode values
under prefers-color-scheme (which an SVG shown as <img> follows from its page).
"""

import datetime
import json
import math
import sys
import urllib.request

DEATH_YEAR = 1951
WORLD_ATLAS = "https://cdn.jsdelivr.net/npm/world-atlas@2.0.2/countries-110m.json"

# ISO 3166 numeric code -> (name, copyright term in years after the author's death), for the
# countries whose term exceeds 70 years. Samoa (75) and Saint Vincent and the Grenadines (75)
# are too small for the 1:110m data.
LONGER_TERMS = {
    "320": ("Guatemala", 75),
    "170": ("Colombia", 80),
    "226": ("Equatorial Guinea", 80),
    "388": ("Jamaica", 95),
    "484": ("Mexico", 100),
}

# Partly in the public domain: in the US the status depends on each item's first publication
# (see the About page's Copyright section). Drawn hatched.
PARTLY = {"840"}  # United States

EXCLUDED = {"010"}  # Antarctica

# Central meridian: ~11°E, as on most world maps, so that the cut through the Pacific falls
# in open sea and the continents sit in the middle.
CENTER_LON = 11

WIDTH = 1000


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


def main():
    if len(sys.argv) > 1:
        with open(sys.argv[1]) as f:
            topo = json.load(f)
    else:
        with urllib.request.urlopen(WORLD_ATLAS) as r:
            topo = json.load(r)

    year = datetime.date.today().year
    copyrighted = {code for code, (_, term) in LONGER_TERMS.items() if DEATH_YEAR + term >= year}

    arcs = decode_arcs(topo)
    countries = []  # (name, status class, [projected rings])
    for g in topo["objects"]["countries"]["geometries"]:
        if g.get("id") in EXCLUDED:
            continue
        rings = [
            [equal_earth(lon, lat) for lon, lat in piece]
            for polygon in polygons(g)
            for ring in polygon
            for piece in split_antimeridian(ring_points(ring, arcs))
        ]
        status = "c" if g.get("id") in copyrighted else "p" if g.get("id") in PARTLY else ""
        countries.append((g["properties"]["name"], status, rings))

    xs = [x for _, _, rings in countries for ring in rings for x, _ in ring]
    ys = [y for _, _, rings in countries for ring in rings for _, y in ring]
    pad = 0.01
    x0, x1, y0, y1 = min(xs) - pad, max(xs) + pad, min(ys) - pad, max(ys) + pad
    k = WIDTH / (x1 - x0)
    height = round((y1 - y0) * k)

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

    out = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {WIDTH} {height}">',
        "<style>",
        "path{fill:#fbcba4;stroke:#fff;stroke-width:0.5;stroke-linejoin:round}",
        "path.c,.s{fill:#444}",
        "path.p{fill:url(#h)}",
        ".b{fill:#fbcba4}",
        "@media (prefers-color-scheme:dark){path{stroke:#262626}path,.b{fill:#624025}path.c,.s{fill:#ccc}}",
        "</style>",
        '<defs><pattern id="h" width="6" height="6" patternUnits="userSpaceOnUse"'
        ' patternTransform="rotate(45)"><rect class="b" width="6" height="6"/>'
        '<rect class="s" width="2" height="6"/></pattern></defs>',
    ]
    for name, status, rings in sorted(countries, key=lambda c: c[0]):
        cls = f' class="{status}"' if status else ""
        out.append(f"<path{cls} d=\"{path(rings)}\"><title>{name}</title></path>")
    out.append("</svg>")
    print("\n".join(out))


if __name__ == "__main__":
    main()
