#!/usr/bin/env python3
"""Neutralize Markdown accidentally triggered by transcription notation.

Reads a document body on stdin, writes the sanitized body to stdout. Only
emphasis/strong and the parser's deliberate scaffolding (headings, links,
images, --- rules, raw HTML) stay live; notation that collides with Markdown
syntax is neutralized:
  - leading ~~~ / ``` runs (code fences, e.g. ~~~~p ⊃ p) -> backslash-escaped
  - leading > (blockquote, Wittgenstein's angle-mark)   -> backslash-escaped
  - * _ ` inside <math> spans                            -> HTML entities

Entities (not backslashes) are used inside math so the fix is identical for
inline and block math: a backslash would show literally in block math (an
opaque HTML block Goldmark passes through verbatim), whereas an entity always
renders as the bare character.
"""

import re
import sys

# Inside <math>, these chars must never be parsed as Markdown (emphasis/code).
MATH_ENTITIES = {"*": "&#42;", "_": "&#95;", "`": "&#96;"}
MATH_SPAN = re.compile(r"(<math\b[^>]*>)(.*?)(</math>)", re.DOTALL)


def _encode_math(match):
    body = match.group(2)
    for ch, ent in MATH_ENTITIES.items():
        body = body.replace(ch, ent)
    return match.group(1) + body + match.group(3)


def sanitize(text):
    text = re.sub(r"(?m)^( {0,3})(~{3,}|`{3,})", r"\1\\\2", text)
    text = re.sub(r"(?m)^( {0,3})>", r"\1\\>", text)
    return MATH_SPAN.sub(_encode_math, text)


if __name__ == "__main__":
    sys.stdout.write(sanitize(sys.stdin.read()))
