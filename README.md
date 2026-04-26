# Wittgenstein's Late Writings

Markdown transcriptions of Ludwig Wittgenstein's late manuscripts and typescripts. These transcriptions are intended to complement the work of the Wittgenstein Archives at the University of Bergen, whose [Bergen Nachlass Edition](https://wab.uib.no/wab_BEE.page) remains the authoritative scholarly edition of the Nachlass.

## Structure

The `md/` directory contains Markdown files for individual documents and published works.

**Documents** are named after their manuscript or typescript number (e.g. `Ms-116.md`, `Ts-228.md`). Each file contains the transcribed remarks of one document, with headings linking to facsimile page images. The heading format is `### [page\[remark\]](facsimile-url)`.

**Works** reassemble remarks from across multiple documents in their published order. They are named with a `W-` prefix (e.g. `W-OC.md` for *On Certainty*, `W-PI.md` for *Philosophical Investigations*). Works with multiple parts use numbered suffixes (e.g. `W-RFM-1.md` through `W-RFM-7.md` for the seven parts of *Remarks on the Foundations of Mathematics*), with a table-of-contents page at the base name (`W-RFM.md`). Work headings include the source document name for reference.

**`index.md`** lists all works and documents.

The `graphics/` directory contains SVG and PNG drawings and diagrams referenced by the Markdown files.

## Mathematical notation

Mathematical expressions are rendered as inline MathML, not LaTeX. Series numbers and section markers use `<span class="series-number">` elements.

## Attribution

### Transcriptions

The transcriptions use the remark numbering established by the Bergen Nachlass Edition. The facsimile images linked from the remark headings are provided by the Wren Library at Trinity College Cambridge, the Austrian National Library, and other institutions.

### Graphics

Most of the drawings and diagrams in the `graphics/` directory come from the [Wittgenstein Nachlass Graphics](https://nachlass-graphics.wittgensteinproject.org/w/index.php/Project:About) project of the Ludwig Wittgenstein Project. These are released under a [Creative Commons Attribution-NonCommercial 4.0 International](https://creativecommons.org/licenses/by-nc/4.0/) license. When reusing these images, please credit the Wittgenstein Nachlass Graphics project and the Ludwig Wittgenstein Project.

A small number of generated graphics (filenames starting with `gen-`) are programmatically produced and are not part of the Nachlass Graphics project. These are released under the same [Creative Commons Attribution-NonCommercial 4.0 International](https://creativecommons.org/licenses/by-nc/4.0/) license.

### Copyright

Wittgenstein's writings are in the public domain in most jurisdictions, including the European Union and countries that apply a 70-year post mortem auctoris term (Wittgenstein died in 1951). In jurisdictions where the works remain under copyright, the rights are held by the Trustees of the Estate of the late Ludwig Wittgenstein, managed through Trinity College, Cambridge. Users should verify the copyright status of these works in their own jurisdiction before reproducing or distributing them.

## Format reference

The files use a subset of CommonMark Markdown with embedded HTML. This section documents all constructs used, so that the files can be processed without a full Markdown parser.

### Markdown constructs

- `# Heading` — document or work title (one per file)
- `## Heading` — section titles within remarks (e.g. chapter headings in the manuscripts)
- `### [text](url)` — remark headings, linking to facsimile page images. Multiple pages are comma-separated: `### [1\[1\]](url),[2\[1\]](url)`. In work files, the first link includes the source document name: `### [Ms-172,1\[1\]](url)`.
- `_text_` — emphasis (italic), used for Wittgenstein's underlinings
- `**text**` — strong emphasis (bold), used where emphasis occurs inside words
- `![](path)` — images, referencing SVG or PNG files in `graphics/`
- `---` — horizontal rules, used to mark divisions within remarks
- `- item` — unordered list items (rare, used in table-of-contents pages and the index)

Blank lines separate paragraphs. Backslash escapes are used for literal brackets in headings: `\[` and `\]`.

### HTML elements

The following HTML elements appear inline within paragraph text:

- `<span class="series-number">N.</span>` — remark or section numbers. The first one in a remark typically marks the remark's position in a numbered series or published work.
- `<math display="inline">...</math>` — inline mathematical expressions in MathML. Uses standard MathML elements: `<mrow>`, `<mi>`, `<mn>`, `<mo>`, `<mtext>`, `<mfrac>`, `<msup>`, `<msub>`, `<msqrt>`, `<mover>`, `<munder>`, `<menclose>`, `<mspace>`, `<mtable>`, `<mtr>`, `<mtd>`. Of these, `<menclose>` is not part of MathML Core and may not render correctly in all browsers. Some expressions carry `class="stacked"` for vertically stacked layouts (e.g. limits).
- `<math display="block">...</math>` — block-level mathematical expressions, on their own line separated by blank lines.
- `<sup>text</sup>` and `<sub>text</sub>` — superscripts and subscripts outside of MathML.
- `<s>text</s>` — strikethrough, representing Wittgenstein's deletions that remain legible.
- `<span class="barover">text</span>` — overline, used for certain notational conventions.
- `<span class="pre-block-punct">text</span>` and `<span class="post-block-punct">text</span>` — punctuation marks that appear immediately before or after a block-level element (image or math block).
- `<table>...</table>` — HTML tables with `<tr>`, `<td>`, and `<th>` elements (rare, used for correspondence tables).
