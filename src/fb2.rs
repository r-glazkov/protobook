use language_tags::LanguageTag;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{
    annotation_element, cite_element, content, epigraph_element, link, poem_element, span,
    title_element, Annotation, AnnotationElement, Author, BaselineShift, Book, Chapter, Cite,
    CiteElement, Content, Date, EmptyLine, Epigraph, EpigraphElement, FontStyle, Footnote,
    FootnoteLink, FootnoteType, Footnotes, Image, InlineImage, Link, Paragraph, Poem, PoemElement,
    Span, Stanza, Table, TableCell, TableRow, Text, TextDecoration, Title, TitleElement,
};

const BOLD_WEIGHT: u32 = 600;

impl Book {
    pub fn from_fb2(
        book: fb2::FictionBook,
        book_id: Uuid,
        binary_ids: &HashMap<String, Uuid>,
    ) -> Book {
        let short_title = book.description.title_info.book_title.value;
        let date = book.description.title_info.date.map(|d| Date {
            iso_date: d.iso_date.map(|date| date.to_string()).unwrap_or_default(),
            display_date: d.display_date.unwrap_or_default(),
        });
        let authors = book
            .description
            .title_info
            .authors
            .into_iter()
            .filter_map(|a| Author::from_fb2(a))
            .collect();
        let language = non_empty(book.description.title_info.lang)
            .filter(|lang| lang.parse::<LanguageTag>().is_ok())
            .unwrap_or_default();
        let cover = book
            .description
            .title_info
            .cover_page
            .into_iter()
            .flat_map(|c| c.images)
            .filter_map(|i| InlineImage::from_fb2(i, binary_ids))
            .next();

        let mut bodies: HashMap<Option<String>, Vec<fb2::Body>> = HashMap::new();
        for body in book.bodies {
            bodies.entry(body.name.clone()).or_default().push(body);
        }
        let body = bodies.remove(&None).unwrap_or_default().into_iter().next();
        let notes = bodies
            .remove(&Some("notes".to_string()))
            .unwrap_or_default()
            .into_iter()
            .next()
            .and_then(|b| {
                Footnotes::from_fb2(
                    b,
                    &Context {
                        binaries: binary_ids,
                        notes: HashSet::new(),
                        comments: HashSet::new(),
                    },
                )
            });
        let comments = bodies
            .remove(&Some("comments".to_string()))
            .unwrap_or_default()
            .into_iter()
            .next()
            .and_then(|b| {
                Footnotes::from_fb2(
                    b,
                    &Context {
                        binaries: binary_ids,
                        notes: HashSet::new(),
                        comments: HashSet::new(),
                    },
                )
            });

        let note_ids = notes
            .as_ref()
            .map(|n| n.content.keys().map(|k| k.to_string()).collect())
            .unwrap_or_default();
        let comment_ids = comments
            .as_ref()
            .map(|c| c.content.keys().map(|k| k.to_string()).collect())
            .unwrap_or_default();
        let ctx = Context {
            binaries: binary_ids,
            notes: note_ids,
            comments: comment_ids,
        };

        let annotation = book
            .description
            .title_info
            .annotation
            .and_then(|a| Annotation::from_fb2(a, &ctx));

        let (chapters, language, title, epigraphs) = if let Some(body) = body {
            let chapters = body
                .sections
                .into_iter()
                .filter_map(|s| Chapter::from_fb2(s, &ctx))
                .collect();
            let language = body.lang.map(|l| l.to_string()).unwrap_or(language);
            let title = body.title.and_then(|t| Title::from_fb2(t, &ctx));
            let epigraphs = body
                .epigraphs
                .into_iter()
                .filter_map(|e| Epigraph::from_fb2(e, &ctx))
                .collect();
            (chapters, language, title, epigraphs)
        } else {
            (vec![], language, None, vec![])
        };

        Book {
            id: book_id.to_string(),
            language,
            short_title,
            date,
            authors,
            cover,
            annotation,
            title,
            epigraphs,
            chapters,
            notes,
            comments,
        }
    }
}

struct Context<'a> {
    binaries: &'a HashMap<String, Uuid>,
    notes: HashSet<String>,
    comments: HashSet<String>,
}

impl Footnotes {
    fn from_fb2(body: fb2::Body, ctx: &Context) -> Option<Footnotes> {
        let content = body
            .sections
            .into_iter()
            .filter_map(|s| Footnote::from_fb2(s, ctx))
            .collect::<HashMap<_, _>>();
        if content.is_empty() {
            return None;
        }
        let title = body.title.and_then(|t| Title::from_fb2(t, ctx));
        Some(Footnotes { title, content })
    }
}

impl Footnote {
    fn from_fb2(value: fb2::Section, ctx: &Context) -> Option<(String, Footnote)> {
        let id = value.id.and_then(non_empty)?;
        let section_content = value.content?;
        let content = section_content
            .content
            .into_iter()
            .filter_map(|c| Content::from_fb2(c, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        let title = section_content.title.and_then(|t| Title::from_fb2(t, ctx));
        Some((id, Footnote { title, content }))
    }
}

impl Author {
    fn from_fb2(value: fb2::Author) -> Option<Author> {
        let (given_name, family_name, middle_name, nickname) = match value {
            fb2::Author::Verbose(a) => (
                non_empty(a.first_name.value),
                non_empty(a.last_name.value),
                a.middle_name.and_then(|n| non_empty(n.value)),
                a.nickname.map(|n| n.value),
            ),
            fb2::Author::Anonymous(a) => (None, None, None, a.nickname.map(|n| n.value)),
        };
        let mut full_name = String::new();
        if let Some(given_name) = &given_name {
            full_name.push_str(given_name);
        }
        if let Some(middle_name) = &middle_name {
            if !full_name.is_empty() {
                full_name.push(' ');
            }
            full_name.push_str(middle_name);
        }
        if let Some(family_name) = &family_name {
            if !full_name.is_empty() {
                full_name.push(' ');
            }
            full_name.push_str(family_name);
        }
        let full_name = if full_name.is_empty() {
            nickname.and_then(non_empty)
        } else {
            Some(full_name)
        };

        if let Some(full_name) = full_name {
            Some(Author {
                id: Uuid::nil().to_string(),
                full_name,
                given_name: given_name.unwrap_or_default(),
                family_name: family_name.unwrap_or_default(),
                middle_name: middle_name.unwrap_or_default(),
            })
        } else {
            None
        }
    }
}

impl Chapter {
    fn from_fb2(section: fb2::Section, ctx: &Context) -> Option<Chapter> {
        let section_content = section.content?;

        let content = section_content
            .content
            .into_iter()
            .filter_map(|part| Content::from_fb2(part, ctx))
            .collect::<Vec<_>>();
        let sub_chapters = section_content
            .sections
            .into_iter()
            .filter_map(|s| Chapter::from_fb2(s, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() && sub_chapters.is_empty() {
            return None;
        }

        let title = section_content.title.and_then(|t| Title::from_fb2(t, ctx));
        let annotation = section_content
            .annotation
            .and_then(|a| Annotation::from_fb2(a, ctx));
        let cover = section_content
            .image
            .and_then(|i| Image::from_fb2(i, ctx.binaries));
        let epigraphs = section_content
            .epigraphs
            .into_iter()
            .filter_map(|e| Epigraph::from_fb2(e, ctx))
            .collect();

        Some(Chapter {
            anchor: section.id.unwrap_or_default(),
            title,
            annotation,
            cover,
            epigraphs,
            content,
            sub_chapters,
        })
    }
}

impl Content {
    fn from_fb2(value: fb2::SectionPart, ctx: &Context) -> Option<Content> {
        match value {
            fb2::SectionPart::Paragraph(p) => Paragraph::from_fb2(p, ctx).map(|p| Content {
                content: Some(content::Content::Paragraph(p)),
            }),
            fb2::SectionPart::Poem(p) => Poem::from_fb2(p, ctx).map(|p| Content {
                content: Some(content::Content::Poem(p)),
            }),
            fb2::SectionPart::Subtitle(p) => Paragraph::from_fb2(p, ctx).map(|s| Content {
                content: Some(content::Content::Subtitle(s)),
            }),
            fb2::SectionPart::Cite(c) => Cite::from_fb2(c, ctx).map(|c| Content {
                content: Some(content::Content::Cite(c)),
            }),
            fb2::SectionPart::Table(t) => Table::from_fb2(t, ctx).map(|t| Content {
                content: Some(content::Content::Table(t)),
            }),
            fb2::SectionPart::Image(i) => Image::from_fb2(i, ctx.binaries).map(|i| Content {
                content: Some(content::Content::Image(i)),
            }),
            fb2::SectionPart::EmptyLine => Some(Content {
                content: Some(content::Content::EmptyLine(EmptyLine {})),
            }),
        }
    }
}

impl Annotation {
    fn from_fb2(value: fb2::Annotation, ctx: &Context) -> Option<Annotation> {
        let content = value
            .elements
            .into_iter()
            .filter_map(|a| AnnotationElement::from_fb2(a, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        Some(Annotation {
            anchor: value.id.unwrap_or_default(),
            content,
        })
    }
}

impl AnnotationElement {
    fn from_fb2(value: fb2::AnnotationElement, ctx: &Context) -> Option<AnnotationElement> {
        match value {
            fb2::AnnotationElement::Paragraph(p) => {
                Paragraph::from_fb2(p, ctx).map(|p| AnnotationElement {
                    annotation_element: Some(annotation_element::AnnotationElement::Paragraph(p)),
                })
            }
            fb2::AnnotationElement::Poem(p) => Poem::from_fb2(p, ctx).map(|p| AnnotationElement {
                annotation_element: Some(annotation_element::AnnotationElement::Poem(p)),
            }),
            fb2::AnnotationElement::Cite(c) => Cite::from_fb2(c, ctx).map(|c| AnnotationElement {
                annotation_element: Some(annotation_element::AnnotationElement::Cite(c)),
            }),
            fb2::AnnotationElement::Subtitle(s) => {
                Paragraph::from_fb2(s, ctx).map(|s| AnnotationElement {
                    annotation_element: Some(annotation_element::AnnotationElement::Subtitle(s)),
                })
            }
            fb2::AnnotationElement::Table(t) => {
                Table::from_fb2(t, ctx).map(|t| AnnotationElement {
                    annotation_element: Some(annotation_element::AnnotationElement::Table(t)),
                })
            }
            fb2::AnnotationElement::EmptyLine => Some(AnnotationElement {
                annotation_element: Some(annotation_element::AnnotationElement::EmptyLine(
                    EmptyLine {},
                )),
            }),
        }
    }
}

impl Epigraph {
    fn from_fb2(value: fb2::Epigraph, ctx: &Context) -> Option<Epigraph> {
        let content = value
            .elements
            .into_iter()
            .filter_map(|e| EpigraphElement::from_fb2(e, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        let authors = value
            .text_authors
            .into_iter()
            .filter_map(|a| Paragraph::from_fb2(a, ctx))
            .collect();
        Some(Epigraph {
            anchor: value.id.unwrap_or_default(),
            authors,
            content,
        })
    }
}

impl EpigraphElement {
    fn from_fb2(value: fb2::EpigraphElement, ctx: &Context) -> Option<EpigraphElement> {
        match value {
            fb2::EpigraphElement::Paragraph(p) => {
                Paragraph::from_fb2(p, ctx).map(|p| EpigraphElement {
                    epigraph_element: Some(epigraph_element::EpigraphElement::Paragraph(p)),
                })
            }
            fb2::EpigraphElement::Poem(p) => Poem::from_fb2(p, ctx).map(|p| EpigraphElement {
                epigraph_element: Some(epigraph_element::EpigraphElement::Poem(p)),
            }),
            fb2::EpigraphElement::Cite(c) => Cite::from_fb2(c, ctx).map(|c| EpigraphElement {
                epigraph_element: Some(epigraph_element::EpigraphElement::Cite(c)),
            }),
            fb2::EpigraphElement::EmptyLine => Some(EpigraphElement {
                epigraph_element: Some(epigraph_element::EpigraphElement::EmptyLine(EmptyLine {})),
            }),
        }
    }
}

impl Poem {
    fn from_fb2(value: fb2::Poem, ctx: &Context) -> Option<Poem> {
        let content = value
            .stanzas
            .into_iter()
            .filter_map(|s| PoemElement::from_fb2(s, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        let title = value.title.and_then(|t| Title::from_fb2(t, ctx));
        let epigraphs = value
            .epigraphs
            .into_iter()
            .filter_map(|e| Epigraph::from_fb2(e, ctx))
            .collect();
        let authors = value
            .text_authors
            .into_iter()
            .filter_map(|a| Paragraph::from_fb2(a, ctx))
            .collect();
        Some(Poem {
            anchor: value.id.unwrap_or_default(),
            title,
            epigraphs,
            authors,
            content,
        })
    }
}

impl PoemElement {
    fn from_fb2(value: fb2::PoemStanza, ctx: &Context) -> Option<PoemElement> {
        match value {
            fb2::PoemStanza::Subtitle(s) => Paragraph::from_fb2(s, ctx).map(|s| PoemElement {
                poem_element: Some(poem_element::PoemElement::Subtitle(s)),
            }),
            fb2::PoemStanza::Stanza(s) => Stanza::from_fb2(s, ctx).map(|s| PoemElement {
                poem_element: Some(poem_element::PoemElement::Stanza(s)),
            }),
        }
    }
}

impl Stanza {
    fn from_fb2(value: fb2::Stanza, ctx: &Context) -> Option<Stanza> {
        let content = value
            .lines
            .into_iter()
            .filter_map(|l| Paragraph::from_fb2(l, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        let title = value.title.and_then(|t| Title::from_fb2(t, ctx));
        let subtitle = value.subtitle.and_then(|s| Paragraph::from_fb2(s, ctx));
        Some(Stanza {
            title,
            subtitle,
            content,
        })
    }
}

impl Cite {
    fn from_fb2(value: fb2::Cite, ctx: &Context) -> Option<Cite> {
        let content = value
            .elements
            .into_iter()
            .filter_map(|c| CiteElement::from_fb2(c, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        let authors = value
            .text_authors
            .into_iter()
            .filter_map(|a| Paragraph::from_fb2(a, ctx))
            .collect();
        Some(Cite {
            anchor: value.id.unwrap_or_default(),
            content,
            authors,
        })
    }
}

impl CiteElement {
    fn from_fb2(value: fb2::CiteElement, ctx: &Context) -> Option<CiteElement> {
        match value {
            fb2::CiteElement::Paragraph(p) => Paragraph::from_fb2(p, ctx).map(|p| CiteElement {
                cite_element: Some(cite_element::CiteElement::Paragraph(p)),
            }),
            fb2::CiteElement::Poem(p) => Poem::from_fb2(p, ctx).map(|p| CiteElement {
                cite_element: Some(cite_element::CiteElement::Poem(p)),
            }),
            fb2::CiteElement::Subtitle(s) => Paragraph::from_fb2(s, ctx).map(|s| CiteElement {
                cite_element: Some(cite_element::CiteElement::Subtitle(s)),
            }),
            fb2::CiteElement::Table(t) => Table::from_fb2(t, ctx).map(|t| CiteElement {
                cite_element: Some(cite_element::CiteElement::Table(t)),
            }),
            fb2::CiteElement::EmptyLine => Some(CiteElement {
                cite_element: Some(cite_element::CiteElement::EmptyLine(EmptyLine {})),
            }),
        }
    }
}

impl Title {
    fn from_fb2(value: fb2::Title, ctx: &Context) -> Option<Title> {
        let content = value
            .elements
            .into_iter()
            .filter_map(|e| TitleElement::from_fb2(e, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }
        Some(Title { content })
    }
}

impl TitleElement {
    fn from_fb2(value: fb2::TitleElement, ctx: &Context) -> Option<TitleElement> {
        match value {
            fb2::TitleElement::Paragraph(p) => Paragraph::from_fb2(p, ctx).map(|p| TitleElement {
                title_element: Some(title_element::TitleElement::Paragraph(p)),
            }),
            fb2::TitleElement::EmptyLine => Some(TitleElement {
                title_element: Some(title_element::TitleElement::EmptyLine(EmptyLine {})),
            }),
        }
    }
}

impl Paragraph {
    fn from_fb2(paragraph: fb2::Paragraph, ctx: &Context) -> Option<Paragraph> {
        let anchor = paragraph.id.unwrap_or_default();
        let content = paragraph
            .elements
            .into_iter()
            .flat_map(|e| Span::from_fb2(e, ctx))
            .collect::<Vec<_>>();
        if content.is_empty() {
            None
        } else {
            Some(Paragraph { anchor, content })
        }
    }
}

impl Table {
    fn from_fb2(value: fb2::Table, ctx: &Context) -> Option<Table> {
        let first_head = value
            .rows
            .first()
            .and_then(|r| r.cells.first())
            .map(|c| matches!(c, fb2::TableCellElement::Head(_)))
            .unwrap_or(false);
        let second_column_head = value
            .rows
            .get(1)
            .and_then(|r| r.cells.first())
            .map(|c| matches!(c, fb2::TableCellElement::Head(_)))
            .unwrap_or(false);
        let second_row_head = value
            .rows
            .first()
            .and_then(|r| r.cells.get(1))
            .map(|c| matches!(c, fb2::TableCellElement::Head(_)))
            .unwrap_or(false);
        let header_column = first_head && second_column_head;
        let header_row = first_head && second_row_head;

        let mut rows = vec![];
        for row in value.rows {
            let cells = row
                .cells
                .into_iter()
                .map(|c| match c {
                    fb2::TableCellElement::Head(h) => h,
                    fb2::TableCellElement::Data(d) => d,
                })
                .map(|c| TableCell::from_fb2(c, ctx))
                .collect::<Vec<_>>();
            if !cells.is_empty() {
                rows.push(TableRow { cells });
            }
        }

        if rows.is_empty() {
            return None;
        }

        Some(Table {
            anchor: value.id.unwrap_or_default(),
            header_column,
            header_row,
            rows,
        })
    }
}

impl TableCell {
    // we don't return Option<TableCell> because it can break layout
    fn from_fb2(value: fb2::TableCell, ctx: &Context) -> TableCell {
        TableCell {
            anchor: value.id.unwrap_or_default(),
            content: value
                .elements
                .into_iter()
                .flat_map(|e| Span::from_fb2(e, ctx))
                .collect(),
        }
    }
}

impl Image {
    fn from_fb2(value: fb2::Image, binary_ids: &HashMap<String, Uuid>) -> Option<Image> {
        value
            .href
            .and_then(non_empty)
            .and_then(|href| href.strip_prefix('#').and_then(|href| binary_ids.get(href)))
            .map(|id| Image {
                id: id.clone().to_string(),
                anchor: value.id.unwrap_or_default(),
                alt: value.alt.unwrap_or_default(),
                title: value.title.unwrap_or_default(),
            })
    }
}

impl Span {
    fn from_fb2(element: fb2::StyleElement, ctx: &Context) -> Vec<Span> {
        let mut spans = vec![];
        match element {
            fb2::StyleElement::Strong(s) => spans.extend(
                s.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(bold_text),
            ),
            fb2::StyleElement::Emphasis(e) => spans.extend(
                e.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(italic_text),
            ),
            fb2::StyleElement::Style(s) => {
                spans.extend(s.elements.into_iter().flat_map(|e| Span::from_fb2(e, ctx)))
            }
            fb2::StyleElement::Link(l) => {
                let href = l.href.and_then(link::Href::from_fb2);
                let content = l
                    .elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, ctx.binaries));

                if let Some(href) = href {
                    let mut images = vec![];
                    let mut text = vec![];
                    for span in content {
                        match span.span.unwrap() {
                            span::Span::Footnote(_) | span::Span::Link(_) => {}
                            span::Span::Image(i) => images.push(Span {
                                span: Some(span::Span::Image(i)),
                            }),
                            span::Span::Text(t) => text.push(t),
                        }
                    }
                    if let Some(id) = ctx.binaries.get(href.as_ref()) {
                        let alt =
                            text.into_iter()
                                .map(|t| t.value)
                                .fold(String::new(), |mut a, b| {
                                    a.push_str(&b);
                                    a
                                });
                        spans.push(Span {
                            span: Some(span::Span::Image(InlineImage {
                                id: id.clone().to_string(),
                                alt: non_empty(alt).unwrap_or_default(),
                            })),
                        });
                    } else if !text.is_empty() {
                        if ctx.notes.contains(href.as_ref()) {
                            let mut footnote_link = FootnoteLink::default();
                            footnote_link.id = href.as_ref().to_string();
                            footnote_link.set_type(FootnoteType::Note);
                            footnote_link.content = text;
                            spans.push(Span {
                                span: Some(span::Span::Footnote(footnote_link)),
                            });
                        } else if ctx.comments.contains(href.as_ref()) {
                            let mut footnote_link = FootnoteLink::default();
                            footnote_link.id = href.as_ref().to_string();
                            footnote_link.set_type(FootnoteType::Comment);
                            footnote_link.content = text;
                            spans.push(Span {
                                span: Some(span::Span::Footnote(footnote_link)),
                            });
                        } else if "note" == l.kind.and_then(non_empty).unwrap_or_default() {
                            spans.extend(text.into_iter().map(|t| Span {
                                span: Some(span::Span::Text(t)),
                            }));
                        } else {
                            spans.push(Span {
                                span: Some(span::Span::Link(Link {
                                    href: Some(href),
                                    content: text,
                                })),
                            });
                        }
                    }
                    spans.extend(images);
                } else {
                    spans.extend(content);
                }
            }
            fb2::StyleElement::Strikethrough(s) => spans.extend(
                s.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(strikethrough_text),
            ),
            fb2::StyleElement::Subscript(s) => spans.extend(
                s.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(subscript_text),
            ),
            fb2::StyleElement::Superscript(s) => spans.extend(
                s.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(superscript_text),
            ),
            fb2::StyleElement::Code(c) => spans.extend(
                c.elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2(e, ctx))
                    .map(code_text),
            ),
            fb2::StyleElement::Image(i) => {
                if let Some(i) = InlineImage::from_fb2(i, ctx.binaries) {
                    spans.push(Span {
                        span: Some(span::Span::Image(i)),
                    });
                }
            }
            fb2::StyleElement::Text(t) => {
                if let Some(text) = Text::from_fb2(t) {
                    spans.push(Span {
                        span: Some(span::Span::Text(text)),
                    });
                }
            }
        }
        spans
    }

    fn from_fb2_link(
        element: fb2::StyleLinkElement,
        binary_ids: &HashMap<String, Uuid>,
    ) -> Vec<Span> {
        let mut spans = vec![];
        match element {
            fb2::StyleLinkElement::Strong { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(bold_text),
            ),
            fb2::StyleLinkElement::Emphasis { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(italic_text),
            ),
            fb2::StyleLinkElement::Style { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids)),
            ),
            fb2::StyleLinkElement::Strikethrough { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(strikethrough_text),
            ),
            fb2::StyleLinkElement::Subscript { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(subscript_text),
            ),
            fb2::StyleLinkElement::Superscript { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(superscript_text),
            ),
            fb2::StyleLinkElement::Code { elements } => spans.extend(
                elements
                    .into_iter()
                    .flat_map(|e| Span::from_fb2_link(e, binary_ids))
                    .map(code_text),
            ),
            fb2::StyleLinkElement::Image(i) => {
                if let Some(i) = InlineImage::from_fb2(i, binary_ids) {
                    spans.push(Span {
                        span: Some(span::Span::Image(i)),
                    });
                }
            }
            fb2::StyleLinkElement::Text(t) => {
                if let Some(text) = Text::from_fb2(t) {
                    spans.push(Span {
                        span: Some(span::Span::Text(text)),
                    });
                }
            }
        }
        spans
    }
}

fn bold_text(span: Span) -> Span {
    hydrate_text(span, |text| text.font_weight = Some(BOLD_WEIGHT))
}

fn subscript_text(span: Span) -> Span {
    hydrate_text(span, |text| {
        text.set_baseline_shift(BaselineShift::Subscript);
    })
}

fn superscript_text(span: Span) -> Span {
    hydrate_text(span, |text| {
        text.set_baseline_shift(BaselineShift::Superscript);
    })
}

fn italic_text(span: Span) -> Span {
    hydrate_text(span, |text| {
        text.set_font_style(FontStyle::Italic);
    })
}

fn code_text(span: Span) -> Span {
    hydrate_text(span, |text| {
        text.set_font_style(FontStyle::Code);
    })
}

fn strikethrough_text(span: Span) -> Span {
    hydrate_text(span, |text| {
        text.push_decorations(TextDecoration::LineThrough);
    })
}

fn hydrate_text(mut span: Span, mut modifier: impl FnMut(&mut Text)) -> Span {
    match span.span.as_mut().unwrap() {
        span::Span::Footnote(f) => {
            for text in &mut f.content {
                modifier(text);
            }
        }
        span::Span::Link(l) => {
            for text in &mut l.content {
                modifier(text);
            }
        }
        span::Span::Image(_) => {}
        span::Span::Text(t) => {
            modifier(t);
        }
    }
    span
}

impl InlineImage {
    fn from_fb2(
        value: fb2::InlineImage,
        binary_ids: &HashMap<String, Uuid>,
    ) -> Option<InlineImage> {
        value
            .href
            .and_then(non_empty)
            .and_then(|href| href.strip_prefix('#').and_then(|href| binary_ids.get(href)))
            .map(|id| InlineImage {
                id: id.clone().to_string(),
                alt: value.alt.unwrap_or_default(),
            })
    }
}

impl link::Href {
    fn from_fb2(href: String) -> Option<link::Href> {
        non_empty(href).and_then(|href| {
            if let Some(href) = href.strip_prefix('#') {
                Some(link::Href::Local(href.to_string()))
            } else if let Ok(url) = href.parse() {
                Some(link::Href::Remote(url))
            } else {
                None
            }
        })
    }
}

impl Text {
    fn from_fb2(value: String) -> Option<Text> {
        non_empty(value).map(|value| Text {
            value,
            font_weight: None,
            font_style: None,
            baseline_shift: None,
            decorations: vec![],
        })
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}
