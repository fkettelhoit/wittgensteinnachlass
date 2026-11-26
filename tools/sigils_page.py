#!/usr/bin/env python3
"""Write the Hugo content of the page explaining the works' sigils (/sigils/).

Usage: sigils_page.py <work_titles.tsv>   (written by the parser to md/: work, german, english)

Each sigil ("OC" for W-OC) is listed with the German and the English title of its work, in
alphabetical order, as a definition list whose terms have the sigil, in lowercase, as id
(`/sigils/#oc`): the "?" after the sigils on the other pages links there
(site/layouts/partials/sigil-help.html), with the titles, kept in the page's front matter
(`titles`: sigil in lowercase → [German, English]), as its tooltip. The titles link to the
work: the German one to
its German page, the English one (`hreflang="en"`) to its German/English page where the
page's layout (site/layouts/_default/sigils.html) finds one.
"""

import csv
import html
import json
import sys

INTRO = (
    "Wittgenstein’s published and edited writings are referred to by sigils. "
    "A Roman numeral after a sigil names a part "
    "(RFM VI), “App” an appendix (PG I App IV), and “MS” the manuscript a part "
    "comes from (LW II MS 169)."
)


def main():
    with open(sys.argv[1], encoding="utf-8", newline="") as f:
        works = list(csv.DictReader(f, delimiter="\t"))
    works.sort(key=lambda w: w["work"])

    lines = []
    tooltips = {}
    for w in works:
        sigil = w["work"].removeprefix("W-")
        slug = sigil.lower()
        german, english = html.escape(w["german"], quote=False), html.escape(w["english"], quote=False)
        href = f"/w-{slug}/"
        # Both titles even where they are the same (Zettel): each links to its own page.
        titles = (
            f'<a lang="de" hreflang="de" href="{href}">{german}</a><br>'
            f'<a class="en" hreflang="en" href="{href}">{english}</a>'
        )
        lines.append(f'<dt id="{slug}">{sigil}</dt><dd>{titles}</dd>')
        tooltips[slug] = [w["german"], w["english"]]

    print("---")
    print('title: "Sigils"')
    print("layout: sigils")
    print(f"titles: {json.dumps(tooltips, ensure_ascii=False)}")
    print("---")
    print()
    print(INTRO)
    print()
    print('<dl class="sigils">')
    print("\n".join(lines))
    print("</dl>")


if __name__ == "__main__":
    main()
