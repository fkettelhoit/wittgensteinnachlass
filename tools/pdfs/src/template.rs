use std::fs;
use std::path::Path;

/// Build the CSS stylesheet with font references via file:// URLs.
pub fn build_css(font_dir: &Path, heading_font_dir: &Path) -> String {
    let font_dir = fs::canonicalize(font_dir)
        .expect("Failed to resolve font directory path");
    let heading_font_dir = fs::canonicalize(heading_font_dir)
        .expect("Failed to resolve heading font directory path");
    let font_url = |dir: &Path, name: &str| -> String {
        let p = dir.join(name);
        format!("file://{}", p.display())
    };

    let regular = font_url(&font_dir, "texgyrepagella-regular.otf");
    let italic = font_url(&font_dir, "texgyrepagella-italic.otf");
    let bold = font_url(&font_dir, "texgyrepagella-bold.otf");
    let bold_italic = font_url(&font_dir, "texgyrepagella-bolditalic.otf");
    let math = font_url(&font_dir, "texgyrepagella-math.otf");

    let sbe_regular = font_url(&heading_font_dir, "SangBleuEmpire-Regular-WebS.woff2");
    let sbe_medium = font_url(&heading_font_dir, "SangBleuEmpire-Medium-WebS.woff2");
    let sbe_bold = font_url(&heading_font_dir, "SangBleuEmpire-Bold-WebS.woff2");
    let sbe_black = font_url(&heading_font_dir, "SangBleuEmpire-Black-WebS.woff2");

    format!(
        r#"
@font-face {{
  font-family: "TeX Gyre Pagella";
  src: url("{regular}") format("opentype");
  font-weight: 400;
  font-style: normal;
}}
@font-face {{
  font-family: "TeX Gyre Pagella";
  src: url("{italic}") format("opentype");
  font-weight: 400;
  font-style: italic;
}}
@font-face {{
  font-family: "TeX Gyre Pagella";
  src: url("{bold}") format("opentype");
  font-weight: 700;
  font-style: normal;
}}
@font-face {{
  font-family: "TeX Gyre Pagella";
  src: url("{bold_italic}") format("opentype");
  font-weight: 700;
  font-style: italic;
}}
@font-face {{
  font-family: "TeX Gyre Pagella Math";
  src: url("{math}") format("opentype");
  font-weight: 400;
  font-style: normal;
}}
@font-face {{
  font-family: "SangBleu Empire";
  src: url("{sbe_regular}") format("woff2");
  font-weight: 400;
  font-style: normal;
}}
@font-face {{
  font-family: "SangBleu Empire";
  src: url("{sbe_medium}") format("woff2");
  font-weight: 500;
  font-style: normal;
}}
@font-face {{
  font-family: "SangBleu Empire";
  src: url("{sbe_bold}") format("woff2");
  font-weight: 700;
  font-style: normal;
}}
@font-face {{
  font-family: "SangBleu Empire";
  src: url("{sbe_black}") format("woff2");
  font-weight: 900;
  font-style: normal;
}}

@page {{
  size: 160mm 240mm;
  margin: 25mm 20mm 30mm 31mm;

  @bottom-center {{
    content: counter(page);
    font-family: "TeX Gyre Pagella", serif;
    font-size: 10pt;
    color: #666;
  }}
}}

@page cover {{
  margin: 1.5mm;
  @bottom-center {{ content: none; }}
}}

@page blank {{
  @bottom-center {{ content: none; }}
}}

@page title {{
  @bottom-center {{ content: none; }}
}}

.cover-page {{
  page: cover;
  page-break-after: always;
  margin: 0;
  padding: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}}

.cover-page img {{
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  display: block;
  margin: auto;
}}

.blank-page {{
  page: blank;
  page-break-after: always;
  height: 240mm;
}}

.title-page {{
  page: title;
  page-break-after: always;
  display: flex;
  flex-direction: column;
  justify-content: center;
  height: 180mm;
}}

.title-page h1 {{
  font-family: "SangBleu Empire", serif;
  font-size: 40pt;
  font-weight: 700;
  text-align: left;
  margin: 0 0 0.4em 0;
  color: #000;
  line-height: 1.05;
  hyphens: none;
}}

.title-page .author-first {{
  font-family: "SangBleu Empire", serif;
  font-size: 22pt;
  font-weight: 400;
  text-align: left;
  color: #000;
  margin: 0;
  line-height: 1.2;
}}

.title-page .author-last {{
  font-family: "SangBleu Empire", serif;
  font-size: 22pt;
  font-weight: 400;
  text-align: left;
  color: #000;
  margin: 0;
  line-height: 1.2;
}}

.content {{
  page-break-before: always;
}}

body {{
  margin: 0;
  padding: 0;
  font-family: "TeX Gyre Pagella", serif;
  font-size: 11pt;
  line-height: 1.6;
  text-align: justify;
  hyphens: auto;
  -webkit-hyphens: auto;
  color: #333;
  orphans: 2;
  widows: 2;
  counter-reset: page;
}}

h1 {{
  font-family: "SangBleu Empire", serif;
  font-size: 48pt;
  font-weight: 700;
  text-align: center;
  margin: 2em 0 1em 0;
  page-break-after: avoid;
}}

h2 {{
  font-family: "SangBleu Empire", serif;
  font-size: 18pt;
  font-weight: 400;
  text-align: center;
  margin: 2em 0 1em 0;
  page-break-before: always;
  page-break-after: avoid;
}}

h3 {{
  float: left;
  clear: left;
  width: 21mm;
  margin-left: -27mm;
  margin-top: 0;
  margin-bottom: 0;
  padding-right: 0;
  font-family: "TeX Gyre Pagella", serif;
  font-size: 11pt;
  font-weight: 400;
  color: #bbb;
  text-align: right;
  line-height: 1.6;
}}

h3 a {{
  color: #bbb;
  text-decoration: none;
}}

.remark {{
  break-inside: avoid;
}}

p {{
  margin: 0 0 1em 0;
  text-indent: 0;
}}

hr {{
  border: none;
  border-top: 0.5pt solid #000;
  margin: 1.5em 0;
}}

blockquote {{
  margin: 0 0 1em 2em;
  font-style: italic;
}}

em, i {{
  font-style: italic;
}}

strong, b {{
  font-weight: 700;
}}

.series-number {{
  background-color: #444;
  color: #fff;
  padding: 0.15em 0.2em 0 0.3em;
  border-radius: 3px;
  margin-right: 0.15em;
  font-size: 0.9em;
}}

math {{
  font-family: "TeX Gyre Pagella Math", math;
}}

sub, sup {{
  line-height: 0;
}}

math[display="block"] {{
  display: block;
  margin: 0.5em 0 1em 0;
}}

s {{
  text-decoration: line-through;
}}

img {{
  max-width: 100%;
  height: auto;
}}
"#
    )
}

/// Build the pandoc HTML template for weasyprint.
pub fn build_template() -> String {
    r#"<!DOCTYPE html>
<html lang="$lang$">
<head>
<meta charset="utf-8">
</head>
<body>
$if(cover-image)$
<div class="cover-page"><img src="$cover-image$"></div>
<div class="blank-page"></div>
$endif$
<div class="title-page">
<h1>$title$</h1>
<p class="author-first">Ludwig</p>
<p class="author-last">Wittgenstein</p>
</div>
<div class="content">
$body$
</div>
</body>
</html>
"#
    .to_string()
}

