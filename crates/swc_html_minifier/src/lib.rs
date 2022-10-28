#![deny(clippy::all)]
#![feature(box_patterns)]

use std::{cmp::Ordering, mem::take};

use once_cell::sync::Lazy;
use serde_json::Value;
use swc_atoms::{js_word, JsWord};
use swc_cached::regex::CachedRegex;
use swc_common::{
    collections::AHashMap, comments::SingleThreadedComments, sync::Lrc, EqIgnoreSpan, FileName,
    FilePathMapping, Mark, SourceMap, DUMMY_SP,
};
use swc_html_ast::*;
use swc_html_parser::parser::ParserConfig;
use swc_html_utils::{HTML_ELEMENTS_AND_ATTRIBUTES, SVG_ELEMENTS_AND_ATTRIBUTES};
use swc_html_visit::{VisitMut, VisitMutWith};

use crate::option::{
    CollapseWhitespaces, CssOptions, JsOptions, JsParserOptions, JsonOptions, MinifierType,
    MinifyCssOption, MinifyJsOption, MinifyJsonOption, MinifyOptions, RemoveRedundantAttributes,
};

pub mod option;

// Global attributes
static EVENT_HANDLER_ATTRIBUTES: &[&str] = &[
    "onabort",
    "onautocomplete",
    "onautocompleteerror",
    "onauxclick",
    "onbeforematch",
    "oncancel",
    "oncanplay",
    "oncanplaythrough",
    "onchange",
    "onclick",
    "onclose",
    "oncontextlost",
    "oncontextmenu",
    "oncontextrestored",
    "oncuechange",
    "ondblclick",
    "ondrag",
    "ondragend",
    "ondragenter",
    "ondragexit",
    "ondragleave",
    "ondragover",
    "ondragstart",
    "ondrop",
    "ondurationchange",
    "onemptied",
    "onended",
    "onformdata",
    "oninput",
    "oninvalid",
    "onkeydown",
    "onkeypress",
    "onkeyup",
    "onmousewheel",
    "onmousedown",
    "onmouseenter",
    "onmouseleave",
    "onmousemove",
    "onmouseout",
    "onmouseover",
    "onmouseup",
    "onpause",
    "onplay",
    "onplaying",
    "onprogress",
    "onratechange",
    "onreset",
    "onsecuritypolicyviolation",
    "onseeked",
    "onseeking",
    "onselect",
    "onslotchange",
    "onstalled",
    "onsubmit",
    "onsuspend",
    "ontimeupdate",
    "ontoggle",
    "onvolumechange",
    "onwaiting",
    "onwebkitanimationend",
    "onwebkitanimationiteration",
    "onwebkitanimationstart",
    "onwebkittransitionend",
    "onwheel",
    "onblur",
    "onerror",
    "onfocus",
    "onload",
    "onloadeddata",
    "onloadedmetadata",
    "onloadstart",
    "onresize",
    "onscroll",
    "onafterprint",
    "onbeforeprint",
    "onbeforeunload",
    "onhashchange",
    "onlanguagechange",
    "onmessage",
    "onmessageerror",
    "onoffline",
    "ononline",
    "onpagehide",
    "onpageshow",
    "onpopstate",
    "onrejectionhandled",
    "onstorage",
    "onunhandledrejection",
    "onunload",
    "oncut",
    "oncopy",
    "onpaste",
    "onreadystatechange",
    "onvisibilitychange",
    "onshow",
    "onsort",
    "onbegin",
    "onend",
    "onrepeat",
];

static ALLOW_TO_TRIM_GLOBAL_ATTRIBUTES: &[&str] = &["style", "tabindex", "itemid"];

static ALLOW_TO_TRIM_HTML_ATTRIBUTES: &[(&str, &str)] = &[
    ("head", "profile"),
    ("audio", "src"),
    ("embed", "src"),
    ("iframe", "src"),
    ("img", "src"),
    ("input", "src"),
    ("input", "usemap"),
    ("input", "longdesc"),
    ("script", "src"),
    ("source", "src"),
    ("track", "src"),
    ("video", "src"),
    ("video", "poster"),
    ("td", "colspan"),
    ("td", "rowspan"),
    ("th", "colspan"),
    ("th", "rowspan"),
    ("col", "span"),
    ("colgroup", "span"),
    ("textarea", "cols"),
    ("textarea", "rows"),
    ("textarea", "maxlength"),
    ("input", "size"),
    ("input", "formaction"),
    ("input", "maxlength"),
    ("button", "formaction"),
    ("select", "size"),
    ("form", "action"),
    ("object", "data"),
    ("object", "codebase"),
    ("object", "classid"),
    ("applet", "codebase"),
    ("a", "href"),
    ("area", "href"),
    ("link", "href"),
    ("base", "href"),
    ("q", "cite"),
    ("blockquote", "cite"),
    ("del", "cite"),
    ("ins", "cite"),
    ("img", "usemap"),
    ("object", "usemap"),
];

static ALLOW_TO_TRIM_SVG_ATTRIBUTES: &[(&str, &str)] = &[("a", "href")];

static COMMA_SEPARATED_HTML_ATTRIBUTES: &[(&str, &str)] = &[
    ("img", "srcset"),
    ("source", "srcset"),
    ("img", "sizes"),
    ("source", "sizes"),
    ("link", "media"),
    ("source", "media"),
    ("style", "media"),
];

static COMMA_SEPARATED_SVG_ATTRIBUTES: &[(&str, &str)] = &[
    ("style", "media"),
    ("polyline", "points"),
    ("polygon", "points"),
];

static SPACE_SEPARATED_GLOBAL_ATTRIBUTES: &[&str] = &[
    "class",
    "itemprop",
    "itemref",
    "itemtype",
    "part",
    "accesskey",
    "aria-describedby",
    "aria-labelledby",
    "aria-owns",
];

static SPACE_SEPARATED_HTML_ATTRIBUTES: &[(&str, &str)] = &[
    ("a", "rel"),
    ("a", "ping"),
    ("area", "rel"),
    ("area", "ping"),
    ("link", "rel"),
    ("link", "sizes"),
    ("link", "blocking"),
    ("iframe", "sandbox"),
    ("td", "headers"),
    ("th", "headers"),
    ("output", "for"),
    ("script", "blocking"),
    ("style", "blocking"),
    ("input", "autocomplete"),
    ("form", "rel"),
    ("form", "autocomplete"),
];

static SPACE_SEPARATED_SVG_ATTRIBUTES: &[(&str, &str)] = &[
    ("svg", "preserveAspectRatio"),
    ("svg", "viewBox"),
    ("symbol", "preserveAspectRatio"),
    ("symbol", "viewBox"),
    ("image", "preserveAspectRatio"),
    ("feImage", "preserveAspectRatio"),
    ("marker", "preserveAspectRatio"),
    ("pattern", "preserveAspectRatio"),
    ("pattern", "viewBox"),
    ("pattern", "patternTransform"),
    ("view", "preserveAspectRatio"),
    ("view", "viewBox"),
    ("path", "d"),
    // TODO improve me more
    ("textPath", "path"),
    ("animateMotion", "path"),
    ("glyph", "d"),
    ("missing-glyph", "d"),
    ("feColorMatrix", "values"),
    ("feConvolveMatrix", "kernelMatrix"),
    ("text", "rotate"),
    ("tspan", "rotate"),
    ("feFuncA", "tableValues"),
    ("feFuncB", "tableValues"),
    ("feFuncG", "tableValues"),
    ("feFuncR", "tableValues"),
    ("linearGradient", "gradientTransform"),
    ("radialGradient", "gradientTransform"),
    ("font-face", "panose-1"),
    ("a", "rel"),
];

static SEMICOLON_SEPARATED_SVG_ATTRIBUTES: &[(&str, &str)] = &[
    ("animate", "keyTimes"),
    ("animate", "keySplines"),
    ("animate", "values"),
    ("animate", "begin"),
    ("animate", "end"),
    ("animateColor", "keyTimes"),
    ("animateColor", "keySplines"),
    ("animateColor", "values"),
    ("animateColor", "begin"),
    ("animateColor", "end"),
    ("animateMotion", "keyTimes"),
    ("animateMotion", "keySplines"),
    ("animateMotion", "values"),
    ("animateMotion", "values"),
    ("animateMotion", "end"),
    ("animateTransform", "keyTimes"),
    ("animateTransform", "keySplines"),
    ("animateTransform", "values"),
    ("animateTransform", "begin"),
    ("animateTransform", "end"),
    ("discard", "begin"),
    ("set", "begin"),
    ("set", "end"),
];

enum CssMinificationMode {
    Stylesheet,
    ListOfDeclarations,
    MediaQueryList,
}

enum HtmlMinificationMode {
    ConditionalComments,
    DocumentIframeSrcdoc,
}

enum HtmlRoot {
    Document(Document),
    DocumentFragment(DocumentFragment),
}

#[inline(always)]
fn is_whitespace(c: char) -> bool {
    matches!(c, '\x09' | '\x0a' | '\x0c' | '\x0d' | '\x20')
}

#[derive(Debug, Copy, Clone)]
struct WhitespaceMinificationMode {
    pub trim: bool,
    pub collapse: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum Display {
    None,
    Inline,
    InlineBlock,
    Block,
    ListItem,
    Ruby,
    RubyBase,
    RubyText,
    Table,
    TableColumnGroup,
    TableCaption,
    TableColumn,
    TableRow,
    TableCell,
    TableHeaderGroup,
    TableRowGroup,
    TableFooterGroup,
    Contents,
}

#[derive(Debug, Eq, PartialEq)]
enum WhiteSpace {
    Pre,
    Normal,
}

pub static CONDITIONAL_COMMENT_START: Lazy<CachedRegex> =
    Lazy::new(|| CachedRegex::new("^\\[if\\s[^\\]+]").unwrap());

pub static CONDITIONAL_COMMENT_END: Lazy<CachedRegex> =
    Lazy::new(|| CachedRegex::new("\\[endif]").unwrap());

struct Minifier<'a> {
    options: &'a MinifyOptions,

    current_element: Option<Element>,
    latest_element: Option<Child>,
    descendant_of_pre: bool,
    attribute_name_counter: Option<AHashMap<JsWord, usize>>,
}

fn get_white_space(namespace: Namespace, tag_name: &JsWord) -> WhiteSpace {
    match namespace {
        Namespace::HTML => match *tag_name {
            js_word!("textarea")
            | js_word!("code")
            | js_word!("pre")
            | js_word!("listing")
            | js_word!("plaintext")
            | js_word!("xmp") => WhiteSpace::Pre,
            _ => WhiteSpace::Normal,
        },
        _ => WhiteSpace::Normal,
    }
}

impl Minifier<'_> {
    fn is_event_handler_attribute(&self, attribute: &Attribute) -> bool {
        EVENT_HANDLER_ATTRIBUTES.contains(&&*attribute.name)
    }

    fn is_boolean_attribute(&self, element: &Element, attribute: &Attribute) -> bool {
        if element.namespace != Namespace::HTML {
            return false;
        }

        if let Some(global_pseudo_element) = HTML_ELEMENTS_AND_ATTRIBUTES.get(&js_word!("*")) {
            if let Some(element) = global_pseudo_element.other.get(&attribute.name) {
                if element.boolean.is_some() && element.boolean.unwrap() {
                    return true;
                }
            }
        }

        if let Some(element) = HTML_ELEMENTS_AND_ATTRIBUTES.get(&element.tag_name) {
            if let Some(element) = element.other.get(&attribute.name) {
                if element.boolean.is_some() && element.boolean.unwrap() {
                    return true;
                }
            }
        }

        false
    }

    fn is_trimable_separated_attribute(&self, element: &Element, attribute: &Attribute) -> bool {
        if ALLOW_TO_TRIM_GLOBAL_ATTRIBUTES.contains(&&*attribute.name) {
            return true;
        }

        match element.namespace {
            Namespace::HTML => {
                ALLOW_TO_TRIM_HTML_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            Namespace::SVG => {
                ALLOW_TO_TRIM_SVG_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            _ => false,
        }
    }

    fn is_comma_separated_attribute(&self, element: &Element, attribute: &Attribute) -> bool {
        match element.namespace {
            Namespace::HTML => match attribute.name {
                js_word!("content")
                    if element.tag_name == js_word!("meta")
                        && (self.element_has_attribute_with_value(
                            element,
                            &js_word!("name"),
                            &[js_word!("viewport"), js_word!("keywords")],
                        )) =>
                {
                    true
                }
                js_word!("imagesrcset")
                    if element.tag_name == js_word!("link")
                        && self.element_has_attribute_with_value(
                            element,
                            &js_word!("rel"),
                            &[js_word!("preload")],
                        ) =>
                {
                    true
                }
                js_word!("imagesizes")
                    if element.tag_name == js_word!("link")
                        && self.element_has_attribute_with_value(
                            element,
                            &js_word!("rel"),
                            &[js_word!("preload")],
                        ) =>
                {
                    true
                }
                js_word!("accept")
                    if element.tag_name == js_word!("input")
                        && self.element_has_attribute_with_value(
                            element,
                            &js_word!("type"),
                            &[js_word!("file")],
                        ) =>
                {
                    true
                }
                _ if attribute.name == js_word!("exportparts") => true,
                _ => {
                    COMMA_SEPARATED_HTML_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
                }
            },
            Namespace::SVG => {
                COMMA_SEPARATED_SVG_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            _ => false,
        }
    }

    fn is_space_separated_attribute(&self, element: &Element, attribute: &Attribute) -> bool {
        if SPACE_SEPARATED_GLOBAL_ATTRIBUTES.contains(&&*attribute.name) {
            return true;
        }

        match element.namespace {
            Namespace::HTML => {
                SPACE_SEPARATED_HTML_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            Namespace::SVG => {
                match attribute.name {
                    js_word!("transform")
                    | js_word!("stroke-dasharray")
                    | js_word!("clip-path")
                    | js_word!("requiredFeatures") => return true,
                    _ => {}
                }

                SPACE_SEPARATED_SVG_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            _ => false,
        }
    }

    fn is_semicolon_separated_attribute(&self, element: &Element, attribute: &Attribute) -> bool {
        match element.namespace {
            Namespace::SVG => {
                SEMICOLON_SEPARATED_SVG_ATTRIBUTES.contains(&(&element.tag_name, &*attribute.name))
            }
            _ => false,
        }
    }

    fn is_attribute_value_unordered_set(&self, element: &Element, attribute: &Attribute) -> bool {
        if matches!(
            attribute.name,
            js_word!("class")
                | js_word!("part")
                | js_word!("itemprop")
                | js_word!("itemref")
                | js_word!("itemtype")
        ) {
            return true;
        }

        match element.namespace {
            Namespace::HTML => match element.tag_name {
                js_word!("link") if attribute.name == js_word!("blocking") => true,
                js_word!("script") if attribute.name == js_word!("blocking") => true,
                js_word!("style") if attribute.name == js_word!("blocking") => true,
                js_word!("output") if attribute.name == js_word!("for") => true,
                js_word!("td") if attribute.name == js_word!("headers") => true,
                js_word!("th") if attribute.name == js_word!("headers") => true,
                js_word!("form") if attribute.name == js_word!("rel") => true,
                js_word!("a") if attribute.name == js_word!("rel") => true,
                js_word!("area") if attribute.name == js_word!("rel") => true,
                js_word!("link") if attribute.name == js_word!("rel") => true,
                js_word!("iframe") if attribute.name == js_word!("sandbox") => true,
                js_word!("link")
                    if self.element_has_attribute_with_value(
                        element,
                        &js_word!("rel"),
                        &[
                            js_word!("icon"),
                            js_word!("apple-touch-icon"),
                            js_word!("apple-touch-icon-precomposed"),
                        ],
                    ) && attribute.name == js_word!("sizes") =>
                {
                    true
                }
                _ => false,
            },
            Namespace::SVG => {
                matches!(element.tag_name, js_word!("a") if attribute.name == js_word!("rel"))
            }
            _ => false,
        }
    }

    fn is_crossorigin_attribute(&self, current_element: &Element, attribute: &Attribute) -> bool {
        matches!(
            (
                current_element.namespace,
                &current_element.tag_name,
                &attribute.name,
            ),
            (
                Namespace::HTML,
                &js_word!("img")
                    | &js_word!("audio")
                    | &js_word!("video")
                    | &js_word!("script")
                    | &js_word!("link"),
                &js_word!("crossorigin"),
            ) | (Namespace::SVG, &js_word!("image"), &js_word!("crossorigin"))
        )
    }

    fn element_has_attribute_with_value(
        &self,
        element: &Element,
        attribute_name: &JsWord,
        attribute_value: &[JsWord],
    ) -> bool {
        element.attributes.iter().any(|attribute| {
            &*attribute.name == attribute_name
                && attribute.value.is_some()
                && attribute_value.contains(&attribute.value.as_ref().unwrap().to_ascii_lowercase())
        })
    }

    fn is_type_text_javascript(&self, value: &JsWord) -> bool {
        let value = value.trim().to_ascii_lowercase();
        let value = if let Some(next) = value.split(';').next() {
            next
        } else {
            &value
        };

        match value {
            // Legacy JavaScript MIME types
            "application/javascript"
            | "application/ecmascript"
            | "application/x-ecmascript"
            | "application/x-javascript"
            | "text/ecmascript"
            | "text/javascript1.0"
            | "text/javascript1.1"
            | "text/javascript1.2"
            | "text/javascript1.3"
            | "text/javascript1.4"
            | "text/javascript1.5"
            | "text/jscript"
            | "text/livescript"
            | "text/x-ecmascript"
            | "text/x-javascript" => true,
            "text/javascript" => true,
            _ => false,
        }
    }

    fn is_default_attribute_value(
        &self,
        namespace: Namespace,
        tag_name: &JsWord,
        attribute: &Attribute,
    ) -> bool {
        let attribute_value = attribute.value.as_ref().unwrap();

        match namespace {
            Namespace::HTML | Namespace::SVG => {
                match *tag_name {
                    js_word!("html") => match attribute.name {
                        js_word!("xmlns") => {
                            if &*attribute_value.trim().to_ascii_lowercase()
                                == "http://www.w3.org/1999/xhtml"
                            {
                                return true;
                            }
                        }
                        js_word!("xmlns:xlink") => {
                            if &*attribute_value.trim().to_ascii_lowercase()
                                == "http://www.w3.org/1999/xlink"
                            {
                                return true;
                            }
                        }
                        _ => {}
                    },
                    js_word!("script") => match attribute.name {
                        js_word!("type") => {
                            if self.is_type_text_javascript(attribute_value) {
                                return true;
                            }
                        }
                        js_word!("language") => {
                            match &*attribute_value.trim().to_ascii_lowercase() {
                                "javascript" | "javascript1.2" | "javascript1.3"
                                | "javascript1.4" | "javascript1.5" | "javascript1.6"
                                | "javascript1.7" => return true,
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                    js_word!("link") => {
                        if attribute.name == js_word!("type")
                            && &*attribute_value.trim().to_ascii_lowercase() == "text/css"
                        {
                            return true;
                        }
                    }

                    js_word!("svg") => {
                        if attribute.name == js_word!("xmlns")
                            && &*attribute_value.trim().to_ascii_lowercase()
                                == "http://www.w3.org/2000/svg"
                        {
                            return true;
                        }
                    }
                    _ => {}
                }

                let default_attributes = if namespace == Namespace::HTML {
                    &HTML_ELEMENTS_AND_ATTRIBUTES
                } else {
                    &SVG_ELEMENTS_AND_ATTRIBUTES
                };

                let attribute_name = if let Some(prefix) = &attribute.prefix {
                    let mut with_namespace =
                        String::with_capacity(prefix.len() + 1 + attribute.name.len());

                    with_namespace.push_str(prefix);
                    with_namespace.push(':');
                    with_namespace.push_str(&attribute.name);

                    with_namespace.into()
                } else {
                    attribute.name.clone()
                };
                let normalized_value = attribute_value.trim();

                let attributes = match default_attributes.get(tag_name) {
                    Some(element) => element,
                    None => return false,
                };

                let attribute_info = match attributes.other.get(&attribute_name) {
                    Some(attribute_info) => attribute_info,
                    None => return false,
                };

                match (attribute_info.inherited, &attribute_info.initial) {
                    (None, Some(initial)) | (Some(false), Some(initial)) => {
                        match self.options.remove_redundant_attributes {
                            RemoveRedundantAttributes::None => false,
                            RemoveRedundantAttributes::Smart => {
                                if initial == normalized_value {
                                    // It is safe to remove deprecated redundant attributes, they
                                    // should not be used
                                    if attribute_info.deprecated == Some(true) {
                                        return true;
                                    }

                                    // It it safe to remove svg redundant attributes, they used for
                                    // styling
                                    if namespace == Namespace::SVG {
                                        return true;
                                    }

                                    // It it safe to remove redundant attributes for metadata
                                    // elements
                                    if namespace == Namespace::HTML
                                        && matches!(
                                            *tag_name,
                                            js_word!("base")
                                                | js_word!("link")
                                                | js_word!("noscript")
                                                | js_word!("script")
                                                | js_word!("style")
                                                | js_word!("title")
                                        )
                                    {
                                        return true;
                                    }
                                }

                                false
                            }
                            RemoveRedundantAttributes::All => initial == normalized_value,
                        }
                    }
                    _ => false,
                }
            }
            _ => {
                matches!(
                    (
                        namespace,
                        tag_name,
                        &attribute.name,
                        attribute_value.to_ascii_lowercase().trim()
                    ),
                    |(
                        Namespace::MATHML,
                        &js_word!("math"),
                        &js_word!("xmlns"),
                        "http://www.w3.org/1998/math/mathml",
                    )| (
                        Namespace::MATHML,
                        &js_word!("math"),
                        &js_word!("xlink"),
                        "http://www.w3.org/1999/xlink"
                    )
                )
            }
        }
    }

    fn is_javascript_url_element(&self, element: &Element) -> bool {
        match (element.namespace, &element.tag_name) {
            (Namespace::HTML | Namespace::SVG, &js_word!("a")) => return true,
            (Namespace::HTML, &js_word!("iframe")) => return true,
            _ => {}
        }

        false
    }

    fn is_preserved_comment(&self, data: &str) -> bool {
        if let Some(preserve_comments) = &self.options.preserve_comments {
            return preserve_comments.iter().any(|regex| regex.is_match(data));
        }

        false
    }

    fn is_conditional_comment(&self, data: &str) -> bool {
        if CONDITIONAL_COMMENT_START.is_match(data) || CONDITIONAL_COMMENT_END.is_match(data) {
            return true;
        }

        false
    }

    fn need_collapse_whitespace(&self) -> bool {
        !matches!(self.options.collapse_whitespaces, CollapseWhitespaces::None)
    }

    fn is_custom_element(&self, tag_name: &JsWord) -> bool {
        // https://html.spec.whatwg.org/multipage/custom-elements.html#valid-custom-element-name
        match *tag_name {
            js_word!("annotation-xml")
            | js_word!("color-profile")
            | js_word!("font-face")
            | js_word!("font-face-src")
            | js_word!("font-face-uri")
            | js_word!("font-face-format")
            | js_word!("font-face-name")
            | js_word!("missing-glyph") => false,
            _ => matches!(tag_name.chars().next(), Some('a'..='z')) && tag_name.contains('-'),
        }
    }

    fn get_display(&self, namespace: Namespace, tag_name: &JsWord) -> Display {
        match namespace {
            Namespace::HTML => match *tag_name {
                js_word!("area")
                | js_word!("base")
                | js_word!("basefont")
                | js_word!("datalist")
                | js_word!("head")
                | js_word!("link")
                | js_word!("meta")
                | js_word!("noembed")
                | js_word!("noframes")
                | js_word!("param")
                | js_word!("rp")
                | js_word!("script")
                | js_word!("style")
                | js_word!("template")
                | js_word!("title") => Display::None,

                js_word!("a")
                | js_word!("abbr")
                | js_word!("acronym")
                | js_word!("b")
                | js_word!("bdi")
                | js_word!("bdo")
                | js_word!("cite")
                | js_word!("data")
                | js_word!("big")
                | js_word!("del")
                | js_word!("dfn")
                | js_word!("em")
                | js_word!("i")
                | js_word!("ins")
                | js_word!("kbd")
                | js_word!("mark")
                | js_word!("q")
                | js_word!("nobr")
                | js_word!("rtc")
                | js_word!("s")
                | js_word!("samp")
                | js_word!("small")
                | js_word!("span")
                | js_word!("strike")
                | js_word!("strong")
                | js_word!("sub")
                | js_word!("sup")
                | js_word!("time")
                | js_word!("tt")
                | js_word!("u")
                | js_word!("var")
                | js_word!("wbr")
                | js_word!("object")
                | js_word!("audio")
                | js_word!("code")
                | js_word!("label")
                | js_word!("br")
                | js_word!("img")
                | js_word!("video")
                | js_word!("noscript")
                | js_word!("picture")
                | js_word!("source")
                | js_word!("track")
                | js_word!("map")
                | js_word!("applet")
                | js_word!("bgsound")
                | js_word!("blink")
                | js_word!("canvas")
                | js_word!("command")
                | js_word!("content")
                | js_word!("embed")
                | js_word!("frame")
                | js_word!("iframe")
                | js_word!("image")
                | js_word!("isindex")
                | js_word!("keygen")
                | js_word!("output")
                | js_word!("rbc")
                | js_word!("shadow")
                | js_word!("spacer") => Display::Inline,

                js_word!("html")
                | js_word!("body")
                | js_word!("address")
                | js_word!("blockquote")
                | js_word!("center")
                | js_word!("div")
                | js_word!("figure")
                | js_word!("figcaption")
                | js_word!("footer")
                | js_word!("form")
                | js_word!("header")
                | js_word!("hr")
                | js_word!("legend")
                | js_word!("listing")
                | js_word!("main")
                | js_word!("p")
                | js_word!("plaintext")
                | js_word!("pre")
                | js_word!("xmp")
                | js_word!("details")
                | js_word!("summary")
                | js_word!("optgroup")
                | js_word!("option")
                | js_word!("h1")
                | js_word!("h2")
                | js_word!("h3")
                | js_word!("h4")
                | js_word!("h5")
                | js_word!("h6")
                | js_word!("fieldset")
                | js_word!("ul")
                | js_word!("ol")
                | js_word!("menu")
                | js_word!("dir")
                | js_word!("dl")
                | js_word!("dt")
                | js_word!("dd")
                | js_word!("section")
                | js_word!("nav")
                | js_word!("hgroup")
                | js_word!("aside")
                | js_word!("article")
                | js_word!("dialog")
                | js_word!("element")
                | js_word!("font")
                | js_word!("frameset") => Display::Block,

                js_word!("li") => Display::ListItem,

                js_word!("button")
                | js_word!("meter")
                | js_word!("progress")
                | js_word!("select")
                | js_word!("textarea")
                | js_word!("input")
                | js_word!("marquee") => Display::InlineBlock,

                js_word!("ruby") => Display::Ruby,

                js_word!("rb") => Display::RubyBase,

                js_word!("rt") => Display::RubyText,

                js_word!("table") => Display::Table,

                js_word!("caption") => Display::TableCaption,

                js_word!("colgroup") => Display::TableColumnGroup,

                js_word!("col") => Display::TableColumn,

                js_word!("thead") => Display::TableHeaderGroup,

                js_word!("tbody") => Display::TableRowGroup,

                js_word!("tfoot") => Display::TableFooterGroup,

                js_word!("tr") => Display::TableRow,

                js_word!("td") | js_word!("th") => Display::TableCell,

                js_word!("slot") => Display::Contents,

                _ => Display::Inline,
            },
            Namespace::SVG => match *tag_name {
                js_word!("text") | js_word!("foreignObject") => Display::Block,
                _ => Display::Inline,
            },
            _ => Display::Inline,
        }
    }

    fn is_element_displayed(&self, namespace: Namespace, tag_name: &JsWord) -> bool {
        match namespace {
            Namespace::HTML => {
                // https://developer.mozilla.org/en-US/docs/Web/Guide/HTML/Content_categories#metadata_content
                //
                // Excluded:
                // `noscript` - can be displayed if JavaScript disabled
                // `script` - can insert markup using `document.write`
                !matches!(
                    *tag_name,
                    js_word!("base")
                        | js_word!("command")
                        | js_word!("link")
                        | js_word!("meta")
                        | js_word!("style")
                        | js_word!("title")
                        | js_word!("template")
                )
            }
            Namespace::SVG => !matches!(*tag_name, js_word!("style")),
            _ => true,
        }
    }

    #[allow(clippy::only_used_in_recursion)]
    fn remove_leading_and_trailing_whitespaces(
        &self,
        children: &mut Vec<Child>,
        only_first: bool,
        only_last: bool,
    ) {
        if only_first {
            if let Some(last) = children.first_mut() {
                match last {
                    Child::Text(text) => {
                        text.data = text.data.trim_start_matches(is_whitespace).into();

                        if text.data.is_empty() {
                            children.remove(0);
                        }
                    }
                    Child::Element(Element {
                        namespace,
                        tag_name,
                        children,
                        ..
                    }) if get_white_space(*namespace, tag_name) == WhiteSpace::Normal => {
                        self.remove_leading_and_trailing_whitespaces(children, true, false);
                    }
                    _ => {}
                }
            }
        }

        if only_last {
            if let Some(last) = children.last_mut() {
                match last {
                    Child::Text(text) => {
                        text.data = text.data.trim_end_matches(is_whitespace).into();

                        if text.data.is_empty() {
                            children.pop();
                        }
                    }
                    Child::Element(Element {
                        namespace,
                        tag_name,
                        children,
                        ..
                    }) if get_white_space(*namespace, tag_name) == WhiteSpace::Normal => {
                        self.remove_leading_and_trailing_whitespaces(children, false, true);
                    }
                    _ => {}
                }
            }
        }
    }

    fn get_prev_displayed_node<'a>(
        &self,
        children: &'a Vec<Child>,
        index: usize,
    ) -> Option<&'a Child> {
        let prev = children.get(index);

        match prev {
            Some(Child::Comment(_)) => {
                if index >= 1 {
                    self.get_prev_displayed_node(children, index - 1)
                } else {
                    None
                }
            }
            Some(Child::Element(element)) => {
                if !self.is_element_displayed(element.namespace, &element.tag_name) && index >= 1 {
                    self.get_prev_displayed_node(children, index - 1)
                } else if !element.children.is_empty() {
                    self.get_prev_displayed_node(&element.children, element.children.len() - 1)
                } else {
                    prev
                }
            }
            Some(_) => prev,
            _ => None,
        }
    }

    fn get_last_displayed_text_node<'a>(
        &self,
        children: &'a Vec<Child>,
        index: usize,
    ) -> Option<&'a Text> {
        let prev = children.get(index);

        match prev {
            Some(Child::Comment(_)) => {
                if index >= 1 {
                    self.get_last_displayed_text_node(children, index - 1)
                } else {
                    None
                }
            }
            Some(Child::Element(element)) => {
                if !self.is_element_displayed(element.namespace, &element.tag_name) && index >= 1 {
                    self.get_last_displayed_text_node(children, index - 1)
                } else if !element.children.is_empty() {
                    for index in (0..=element.children.len() - 1).rev() {
                        if let Some(text) =
                            self.get_last_displayed_text_node(&element.children, index)
                        {
                            return Some(text);
                        }
                    }

                    None
                } else {
                    None
                }
            }
            Some(Child::Text(text)) => Some(text),
            _ => None,
        }
    }

    fn get_first_displayed_text_node<'a>(
        &self,
        children: &'a Vec<Child>,
        index: usize,
    ) -> Option<&'a Text> {
        let next = children.get(index);

        match next {
            Some(Child::Comment(_)) => self.get_first_displayed_text_node(children, index + 1),
            Some(Child::Element(element)) => {
                if !self.is_element_displayed(element.namespace, &element.tag_name) && index >= 1 {
                    self.get_first_displayed_text_node(children, index - 1)
                } else if !element.children.is_empty() {
                    for index in 0..=element.children.len() - 1 {
                        if let Some(text) =
                            self.get_first_displayed_text_node(&element.children, index)
                        {
                            return Some(text);
                        }
                    }

                    None
                } else {
                    None
                }
            }
            Some(Child::Text(text)) => Some(text),
            _ => None,
        }
    }

    fn get_next_displayed_node<'a>(
        &self,
        children: &'a Vec<Child>,
        index: usize,
    ) -> Option<&'a Child> {
        let next = children.get(index);

        match next {
            Some(Child::Comment(_)) => self.get_next_displayed_node(children, index + 1),
            Some(Child::Element(element))
                if !self.is_element_displayed(element.namespace, &element.tag_name) =>
            {
                self.get_next_displayed_node(children, index + 1)
            }
            Some(_) => next,
            _ => None,
        }
    }

    fn get_whitespace_minification_for_tag(
        &self,
        namespace: Namespace,
        tag_name: &JsWord,
    ) -> WhitespaceMinificationMode {
        let default_collapse = match self.options.collapse_whitespaces {
            CollapseWhitespaces::All
            | CollapseWhitespaces::Smart
            | CollapseWhitespaces::Conservative
            | CollapseWhitespaces::AdvancedConservative => true,
            CollapseWhitespaces::OnlyMetadata | CollapseWhitespaces::None => false,
        };
        let default_trim = match self.options.collapse_whitespaces {
            CollapseWhitespaces::All => true,
            CollapseWhitespaces::Smart
            | CollapseWhitespaces::Conservative
            | CollapseWhitespaces::OnlyMetadata
            | CollapseWhitespaces::AdvancedConservative
            | CollapseWhitespaces::None => false,
        };

        match namespace {
            Namespace::HTML => match *tag_name {
                js_word!("script") | js_word!("style") => WhitespaceMinificationMode {
                    collapse: false,
                    trim: !matches!(
                        self.options.collapse_whitespaces,
                        CollapseWhitespaces::None | CollapseWhitespaces::OnlyMetadata
                    ),
                },
                _ => {
                    if get_white_space(namespace, tag_name) == WhiteSpace::Pre {
                        WhitespaceMinificationMode {
                            collapse: false,
                            trim: false,
                        }
                    } else {
                        WhitespaceMinificationMode {
                            collapse: default_collapse,
                            trim: default_trim,
                        }
                    }
                }
            },
            Namespace::SVG => match *tag_name {
                js_word!("script") | js_word!("style") => WhitespaceMinificationMode {
                    collapse: false,
                    trim: true,
                },
                // https://svgwg.org/svg2-draft/render.html#Definitions
                _ if matches!(
                    *tag_name,
                    js_word!("a")
                        | js_word!("circle")
                        | js_word!("ellipse")
                        | js_word!("foreignObject")
                        | js_word!("g")
                        | js_word!("image")
                        | js_word!("line")
                        | js_word!("path")
                        | js_word!("polygon")
                        | js_word!("polyline")
                        | js_word!("rect")
                        | js_word!("svg")
                        | js_word!("switch")
                        | js_word!("symbol")
                        | js_word!("text")
                        | js_word!("textPath")
                        | js_word!("tspan")
                        | js_word!("use")
                ) =>
                {
                    WhitespaceMinificationMode {
                        collapse: default_collapse,
                        trim: default_trim,
                    }
                }
                _ => WhitespaceMinificationMode {
                    collapse: default_collapse,
                    trim: !matches!(
                        self.options.collapse_whitespaces,
                        CollapseWhitespaces::None | CollapseWhitespaces::OnlyMetadata
                    ),
                },
            },
            _ => WhitespaceMinificationMode {
                collapse: false,
                trim: default_trim,
            },
        }
    }

    fn collapse_whitespace(&self, data: &str) -> String {
        let mut collapsed = String::with_capacity(data.len());
        let mut in_whitespace = false;

        for c in data.chars() {
            if is_whitespace(c) {
                if in_whitespace {
                    // Skip this character.
                    continue;
                };

                in_whitespace = true;

                collapsed.push(' ');
            } else {
                in_whitespace = false;

                collapsed.push(c);
            };
        }

        collapsed
    }

    fn is_additional_minifier_attribute(&self, name: &str) -> Option<MinifierType> {
        if let Some(minify_additional_attributes) = &self.options.minify_additional_attributes {
            for item in minify_additional_attributes {
                if item.0.is_match(name) {
                    return Some(item.1.clone());
                }
            }
        }

        None
    }

    fn is_empty_metadata_element(&self, child: &Child) -> bool {
        if let Child::Element(element) = child {
            if (!self.is_element_displayed(element.namespace, &element.tag_name)
                || (matches!(element.namespace, Namespace::HTML | Namespace::SVG)
                    && element.tag_name == js_word!("script"))
                || (element.namespace == Namespace::HTML
                    && element.tag_name == js_word!("noscript")))
                && element.attributes.is_empty()
                && self.is_empty_children(&element.children)
                && element.content.is_none()
            {
                return true;
            }
        }

        false
    }

    fn is_empty_children(&self, children: &Vec<Child>) -> bool {
        for child in children {
            match child {
                Child::Text(text) if text.data.chars().all(is_whitespace) => {
                    continue;
                }
                _ => return false,
            }
        }

        true
    }

    fn allow_elements_to_merge(&self, left: Option<&Child>, right: &Element) -> bool {
        if let Some(Child::Element(left)) = left {
            let is_style_tag = matches!(left.namespace, Namespace::HTML | Namespace::SVG)
                && left.tag_name == js_word!("style")
                && matches!(right.namespace, Namespace::HTML | Namespace::SVG)
                && right.tag_name == js_word!("style");
            let is_script_tag = matches!(left.namespace, Namespace::HTML | Namespace::SVG)
                && left.tag_name == js_word!("script")
                && matches!(right.namespace, Namespace::HTML | Namespace::SVG)
                && right.tag_name == js_word!("script");

            if is_style_tag || is_script_tag {
                let mut need_skip = false;

                let mut left_attributes = left
                    .attributes
                    .clone()
                    .into_iter()
                    .filter(|attribute| match attribute.name {
                        js_word!("src") if is_script_tag => {
                            need_skip = true;

                            true
                        }
                        js_word!("type") => {
                            if let Some(value) = &attribute.value {
                                if (is_style_tag && value.trim().to_ascii_lowercase() == "text/css")
                                    || is_script_tag && self.is_type_text_javascript(value)
                                {
                                    false
                                } else {
                                    need_skip = true;

                                    true
                                }
                            } else {
                                true
                            }
                        }
                        _ => true,
                    })
                    .collect::<Vec<Attribute>>();

                if need_skip {
                    return false;
                }

                let mut right_attributes = right
                    .attributes
                    .clone()
                    .into_iter()
                    .filter(|attribute| match attribute.name {
                        js_word!("src") if is_script_tag => {
                            need_skip = true;

                            true
                        }
                        js_word!("type") => {
                            if let Some(value) = &attribute.value {
                                if (is_style_tag && value.trim().to_ascii_lowercase() == "text/css")
                                    || (is_script_tag && self.is_type_text_javascript(value))
                                {
                                    false
                                } else {
                                    need_skip = true;

                                    true
                                }
                            } else {
                                true
                            }
                        }
                        _ => true,
                    })
                    .collect::<Vec<Attribute>>();

                if need_skip {
                    return false;
                }

                left_attributes.sort_by(|a, b| a.name.cmp(&b.name));
                right_attributes.sort_by(|a, b| a.name.cmp(&b.name));

                return left_attributes.eq_ignore_span(&right_attributes);
            }
        }

        false
    }

    fn merge_text_children(&self, left: &Element, right: &Element) -> Vec<Child> {
        let is_script_tag = matches!(left.namespace, Namespace::HTML | Namespace::SVG)
            && left.tag_name == js_word!("script")
            && matches!(right.namespace, Namespace::HTML | Namespace::SVG)
            && right.tag_name == js_word!("script");

        let data = left.children.iter().chain(right.children.iter()).fold(
            String::new(),
            |mut acc, child| match child {
                Child::Text(text) if text.data.len() > 0 => {
                    acc.push_str(&text.data);

                    if is_script_tag {
                        acc.push(';');
                    }

                    acc
                }
                _ => acc,
            },
        );

        if data.is_empty() {
            return vec![];
        }

        vec![Child::Text(Text {
            span: DUMMY_SP,
            data: data.into(),
            raw: None,
        })]
    }

    fn minify_children(&mut self, children: &mut Vec<Child>) -> Vec<Child> {
        if children.is_empty() {
            return vec![];
        }

        let (namespace, tag_name) = match &self.current_element {
            Some(element) => (element.namespace, &element.tag_name),
            _ => {
                unreachable!();
            }
        };

        let mode = self.get_whitespace_minification_for_tag(namespace, tag_name);

        let child_will_be_retained =
            |child: &mut Child, prev_children: &mut Vec<Child>, next_children: &mut Vec<Child>| {
                match child {
                    Child::Comment(comment) if self.options.remove_comments => {
                        self.is_preserved_comment(&comment.data)
                    }
                    Child::Element(element)
                        if self.options.merge_metadata_elements
                            && self.allow_elements_to_merge(prev_children.last(), element) =>
                    {
                        if let Some(Child::Element(prev)) = prev_children.last_mut() {
                            prev.children = self.merge_text_children(prev, element);
                        }

                        false
                    }
                    Child::Text(text) if text.data.is_empty() => false,
                    Child::Text(text)
                        if self.need_collapse_whitespace()
                            && namespace == Namespace::HTML
                            && matches!(*tag_name, js_word!("html") | js_word!("head"))
                            && text.data.chars().all(is_whitespace) =>
                    {
                        false
                    }
                    Child::Text(text)
                        if !self.descendant_of_pre
                            && get_white_space(namespace, tag_name) == WhiteSpace::Normal
                            && matches!(
                                self.options.collapse_whitespaces,
                                CollapseWhitespaces::All
                                    | CollapseWhitespaces::Smart
                                    | CollapseWhitespaces::OnlyMetadata
                                    | CollapseWhitespaces::Conservative
                                    | CollapseWhitespaces::AdvancedConservative
                            ) =>
                    {
                        let mut is_smart_left_trim = false;
                        let mut is_smart_right_trim = false;

                        if matches!(
                            self.options.collapse_whitespaces,
                            CollapseWhitespaces::Smart
                                | CollapseWhitespaces::OnlyMetadata
                                | CollapseWhitespaces::AdvancedConservative
                        ) {
                            let need_remove_metadata_whitespaces = matches!(
                                self.options.collapse_whitespaces,
                                CollapseWhitespaces::OnlyMetadata
                                    | CollapseWhitespaces::AdvancedConservative
                            );

                            let prev = prev_children.last();
                            let prev_display = match prev {
                                Some(Child::Element(Element {
                                    namespace,
                                    tag_name,
                                    ..
                                })) => Some(self.get_display(*namespace, tag_name)),
                                Some(Child::Comment(_)) => match need_remove_metadata_whitespaces {
                                    true => None,
                                    _ => Some(Display::None),
                                },
                                _ => None,
                            };

                            let allow_to_trim_left = match prev_display {
                                // Block-level containers:
                                //
                                // `Display::Block`    - `display: block flow`
                                // `Display::ListItem` - `display: block flow list-item`
                                // `Display::Table`    - `display: block table`
                                //
                                // + internal table display (only whitespace characters allowed
                                // there)
                                Some(
                                    Display::Block
                                    | Display::ListItem
                                    | Display::Table
                                    | Display::TableColumnGroup
                                    | Display::TableCaption
                                    | Display::TableColumn
                                    | Display::TableRow
                                    | Display::TableCell
                                    | Display::TableHeaderGroup
                                    | Display::TableRowGroup
                                    | Display::TableFooterGroup,
                                ) => true,
                                // Elements are not displayed
                                // And
                                // Inline box
                                Some(Display::None) | Some(Display::Inline) => {
                                    // A custom element can contain any elements, we cannot predict
                                    // the behavior of spaces
                                    let is_custom_element =
                                        if let Some(Child::Element(element)) = &prev {
                                            self.is_custom_element(&element.tag_name)
                                        } else {
                                            false
                                        };

                                    if is_custom_element {
                                        false
                                    } else {
                                        match &self.get_prev_displayed_node(
                                            prev_children,
                                            prev_children.len() - 1,
                                        ) {
                                            Some(Child::Text(text)) => {
                                                text.data.ends_with(is_whitespace)
                                            }
                                            Some(Child::Element(element)) => {
                                                let deep = if !element.children.is_empty() {
                                                    self.get_last_displayed_text_node(
                                                        &element.children,
                                                        element.children.len() - 1,
                                                    )
                                                } else {
                                                    None
                                                };

                                                if let Some(deep) = deep {
                                                    deep.data.ends_with(is_whitespace)
                                                } else {
                                                    false
                                                }
                                            }
                                            _ => {
                                                let parent_display =
                                                    self.get_display(namespace, tag_name);

                                                match parent_display {
                                                    Display::Inline => {
                                                        if let Some(Child::Text(Text {
                                                            data,
                                                            ..
                                                        })) = &self.latest_element
                                                        {
                                                            data.ends_with(is_whitespace)
                                                        } else {
                                                            false
                                                        }
                                                    }
                                                    _ => true,
                                                }
                                            }
                                        }
                                    }
                                }
                                // Inline level containers and etc
                                Some(_) => false,
                                None => {
                                    // Template can be used in any place, so let's keep whitespaces
                                    //
                                    // For custom elements - an unnamed `<slot>` will be filled with
                                    // all of the custom
                                    // element's top-level child
                                    // nodes that do not have the slot
                                    // attribute. This includes text nodes.
                                    // Also they can be used for custom logic

                                    if (namespace == Namespace::HTML
                                        && *tag_name == js_word!("template"))
                                        || self.is_custom_element(tag_name)
                                    {
                                        false
                                    } else {
                                        let parent_display = self.get_display(namespace, tag_name);

                                        match parent_display {
                                            Display::Inline => {
                                                if let Some(Child::Text(Text { data, .. })) =
                                                    &self.latest_element
                                                {
                                                    data.ends_with(is_whitespace)
                                                } else {
                                                    false
                                                }
                                            }
                                            _ => true,
                                        }
                                    }
                                }
                            };

                            let next = next_children.first();
                            let next_display = match next {
                                Some(Child::Element(Element {
                                    namespace,
                                    tag_name,
                                    ..
                                })) => Some(self.get_display(*namespace, tag_name)),
                                Some(Child::Comment(_)) => match need_remove_metadata_whitespaces {
                                    true => None,
                                    _ => Some(Display::None),
                                },
                                _ => None,
                            };

                            let allow_to_trim_right = match next_display {
                                // Block-level containers:
                                //
                                // `Display::Block`    - `display: block flow`
                                // `Display::ListItem` - `display: block flow list-item`
                                // `Display::Table`    - `display: block table`
                                //
                                // + internal table display (only whitespace characters allowed
                                // there)
                                Some(
                                    Display::Block
                                    | Display::ListItem
                                    | Display::Table
                                    | Display::TableColumnGroup
                                    | Display::TableCaption
                                    | Display::TableColumn
                                    | Display::TableRow
                                    | Display::TableCell
                                    | Display::TableHeaderGroup
                                    | Display::TableRowGroup
                                    | Display::TableFooterGroup,
                                ) => true,
                                // These elements are not displayed
                                Some(Display::None) => {
                                    match &self.get_next_displayed_node(next_children, 0) {
                                        Some(Child::Text(text)) => {
                                            text.data.starts_with(is_whitespace)
                                        }
                                        Some(Child::Element(element)) => {
                                            let deep = self.get_first_displayed_text_node(
                                                &element.children,
                                                0,
                                            );

                                            if let Some(deep) = deep {
                                                !deep.data.starts_with(is_whitespace)
                                            } else {
                                                false
                                            }
                                        }
                                        _ => {
                                            let parent_display =
                                                self.get_display(namespace, tag_name);

                                            !matches!(parent_display, Display::Inline)
                                        }
                                    }
                                }
                                Some(_) => false,
                                None => {
                                    // Template can be used in any place, so let's keep whitespaces
                                    let is_template = namespace == Namespace::HTML
                                        && *tag_name == js_word!("template");

                                    if is_template {
                                        false
                                    } else {
                                        let parent_display = self.get_display(namespace, tag_name);

                                        !matches!(parent_display, Display::Inline)
                                    }
                                }
                            };

                            if matches!(
                                self.options.collapse_whitespaces,
                                CollapseWhitespaces::Smart
                            ) || (need_remove_metadata_whitespaces
                                && (prev_display == Some(Display::None)
                                    && next_display == Some(Display::None)))
                            {
                                is_smart_left_trim = allow_to_trim_left;
                                is_smart_right_trim = allow_to_trim_right;
                            }
                        }

                        let mut value = if (mode.trim) || is_smart_left_trim {
                            text.data.trim_start_matches(is_whitespace)
                        } else {
                            &*text.data
                        };

                        value = if (mode.trim) || is_smart_right_trim {
                            value.trim_end_matches(is_whitespace)
                        } else {
                            value
                        };

                        if value.is_empty() {
                            false
                        } else if mode.collapse {
                            text.data = self.collapse_whitespace(value).into();

                            true
                        } else {
                            text.data = value.into();

                            true
                        }
                    }
                    _ => true,
                }
            };

        let mut new_children = Vec::with_capacity(children.len());

        for _ in 0..children.len() {
            let mut child = children.remove(0);

            if let Child::Text(text) = &mut child {
                if let Some(Child::Text(prev_text)) = new_children.last_mut() {
                    let mut new_data =
                        String::with_capacity(prev_text.data.len() + text.data.len());

                    new_data.push_str(&prev_text.data);
                    new_data.push_str(&text.data);

                    text.data = new_data.into();

                    new_children.pop();
                }
            };

            let result = child_will_be_retained(&mut child, &mut new_children, children);

            if result {
                if self.options.remove_empty_metadata_elements
                    && self.is_empty_metadata_element(&child)
                {
                    let need_continue = {
                        let next_element = if let Some(Child::Element(element)) = children.get(0) {
                            Some(element)
                        } else if let Some(Child::Element(element)) = children.get(1) {
                            Some(element)
                        } else {
                            None
                        };

                        if let Some(element) = next_element {
                            self.options.merge_metadata_elements
                                && !self.allow_elements_to_merge(Some(&child), element)
                        } else {
                            true
                        }
                    };

                    if need_continue {
                        continue;
                    }
                }

                new_children.push(child);
            }
        }

        new_children
    }

    fn get_attribute_value<'a>(
        &self,
        attributes: &'a Vec<Attribute>,
        name: JsWord,
    ) -> Option<&'a JsWord> {
        let mut type_attribute_value = None;

        for attribute in attributes {
            if attribute.name == name {
                if let Some(value) = &attribute.value {
                    type_attribute_value = Some(value);
                }

                break;
            }
        }

        type_attribute_value
    }

    fn is_additional_scripts_content(&self, name: &str) -> Option<MinifierType> {
        if let Some(minify_additional_scripts_content) =
            &self.options.minify_additional_scripts_content
        {
            for item in minify_additional_scripts_content {
                if item.0.is_match(name) {
                    return Some(item.1.clone());
                }
            }
        }

        None
    }

    fn need_minify_json(&self) -> bool {
        match self.options.minify_json {
            MinifyJsonOption::Bool(value) => value,
            MinifyJsonOption::Options(_) => true,
        }
    }

    fn get_json_options(&self) -> JsonOptions {
        match &self.options.minify_json {
            MinifyJsonOption::Bool(_) => JsonOptions { pretty: false },
            MinifyJsonOption::Options(json_options) => *json_options.clone(),
        }
    }

    fn minify_json(&self, data: String) -> Option<String> {
        let json = match serde_json::from_str::<Value>(&data) {
            Ok(json) => json,
            _ => return None,
        };

        let options = self.get_json_options();
        let result = match options.pretty {
            true => serde_json::to_string_pretty(&json),
            false => serde_json::to_string(&json),
        };

        match result {
            Ok(minified_json) => Some(minified_json),
            _ => None,
        }
    }

    fn need_minify_js(&self) -> bool {
        match self.options.minify_js {
            MinifyJsOption::Bool(value) => value,
            MinifyJsOption::Options(_) => true,
        }
    }

    fn get_js_options(&self) -> JsOptions {
        match &self.options.minify_js {
            MinifyJsOption::Bool(_) => JsOptions {
                parser: JsParserOptions::default(),
                minifier: swc_ecma_minifier::option::MinifyOptions {
                    compress: Some(swc_ecma_minifier::option::CompressOptions::default()),
                    mangle: Some(swc_ecma_minifier::option::MangleOptions::default()),
                    ..Default::default()
                },
                codegen: swc_ecma_codegen::Config::default(),
            },
            MinifyJsOption::Options(js_options) => *js_options.clone(),
        }
    }

    // TODO source map url output for JS and CSS?
    fn minify_js(&self, data: String, is_module: bool, is_attribute: bool) -> Option<String> {
        let mut errors: Vec<_> = vec![];

        let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
        let fm = cm.new_source_file(FileName::Anon, data);
        let mut options = self.get_js_options();

        if let swc_ecma_parser::Syntax::Es(es_config) = &mut options.parser.syntax {
            es_config.allow_return_outside_function = !is_module && is_attribute;
        }

        let comments = SingleThreadedComments::default();

        let mut program = if is_module {
            match swc_ecma_parser::parse_file_as_module(
                &fm,
                options.parser.syntax,
                options.parser.target,
                if options.parser.comments {
                    Some(&comments)
                } else {
                    None
                },
                &mut errors,
            ) {
                Ok(module) => swc_ecma_ast::Program::Module(module),
                _ => return None,
            }
        } else {
            match swc_ecma_parser::parse_file_as_script(
                &fm,
                options.parser.syntax,
                options.parser.target,
                if options.parser.comments {
                    Some(&comments)
                } else {
                    None
                },
                &mut errors,
            ) {
                Ok(script) => swc_ecma_ast::Program::Script(script),
                _ => return None,
            }
        };

        // Avoid compress potential invalid JS
        if !errors.is_empty() {
            return None;
        }

        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        if let Some(compress_options) = &mut options.minifier.compress {
            compress_options.module = is_module;
        } else {
            options.minifier.compress = Some(swc_ecma_minifier::option::CompressOptions {
                ecma: options.parser.target,
                ..Default::default()
            });
        }

        swc_ecma_visit::VisitMutWith::visit_mut_with(
            &mut program,
            &mut swc_ecma_transforms_base::resolver(unresolved_mark, top_level_mark, false),
        );

        let program = swc_ecma_minifier::optimize(
            program,
            cm.clone(),
            if options.parser.comments {
                Some(&comments)
            } else {
                None
            },
            None,
            &options.minifier,
            &swc_ecma_minifier::option::ExtraOptions {
                unresolved_mark,
                top_level_mark,
            },
        );

        let program = swc_ecma_visit::FoldWith::fold_with(
            program,
            &mut swc_ecma_transforms_base::fixer::fixer(Some(&comments)),
        );

        let mut buf = vec![];

        {
            let mut wr = Box::new(swc_ecma_codegen::text_writer::JsWriter::new(
                cm.clone(),
                "\n",
                &mut buf,
                None,
            )) as Box<dyn swc_ecma_codegen::text_writer::WriteJs>;

            wr = Box::new(swc_ecma_codegen::text_writer::omit_trailing_semi(wr));

            options.codegen.minify = true;
            options.codegen.target = options.parser.target;
            options.codegen.omit_last_semi = true;

            let mut emitter = swc_ecma_codegen::Emitter {
                cfg: options.codegen,
                cm,
                comments: if options.parser.comments {
                    Some(&comments)
                } else {
                    None
                },
                wr,
            };

            emitter.emit_program(&program).unwrap();
        }

        let minified = match String::from_utf8(buf) {
            // Avoid generating the sequence "</script" in JS code
            // TODO move it to ecma codegen under the option?
            Ok(minified) => minified.replace("</script>", "<\\/script>"),
            _ => return None,
        };

        Some(minified)
    }

    fn need_minify_css(&self) -> bool {
        match self.options.minify_css {
            MinifyCssOption::Bool(value) => value,
            MinifyCssOption::Options(_) => true,
        }
    }

    fn minify_sizes(&self, value: &str) -> Option<String> {
        let values = value
            .rsplitn(2, |c| matches!(c, '\t' | '\n' | '\x0C' | '\r' | ' '))
            .collect::<Vec<&str>>();

        if values.len() != 2 {
            return None;
        }

        if !values[1].starts_with('(') {
            return None;
        }

        let media_condition =
            // It should be `MediaCondition`, but `<media-query> = <media-condition>` and other values is just invalid size
            match self.minify_css(values[1].to_string(), CssMinificationMode::MediaQueryList) {
                Some(minified) => minified,
                _ => return None,
            };

        let source_size_value = values[0];
        let mut minified = String::with_capacity(media_condition.len() + source_size_value.len());

        minified.push_str(&media_condition);
        minified.push(' ');
        minified.push_str(source_size_value);

        Some(minified)
    }

    fn get_css_options(&self) -> CssOptions {
        match &self.options.minify_css {
            MinifyCssOption::Bool(_) => CssOptions {
                parser: swc_css_parser::parser::ParserConfig::default(),
                minifier: swc_css_minifier::options::MinifyOptions::default(),
                codegen: swc_css_codegen::CodegenConfig::default(),
            },
            MinifyCssOption::Options(css_options) => *css_options.clone(),
        }
    }

    fn minify_css(&self, data: String, mode: CssMinificationMode) -> Option<String> {
        let mut errors: Vec<_> = vec![];

        let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
        let fm = cm.new_source_file(FileName::Anon, data);

        let mut options = self.get_css_options();

        let mut stylesheet = match mode {
            CssMinificationMode::Stylesheet => {
                match swc_css_parser::parse_file(&fm, options.parser, &mut errors) {
                    Ok(stylesheet) => stylesheet,
                    _ => return None,
                }
            }
            CssMinificationMode::ListOfDeclarations => {
                match swc_css_parser::parse_file::<Vec<swc_css_ast::DeclarationOrAtRule>>(
                    &fm,
                    options.parser,
                    &mut errors,
                ) {
                    Ok(list_of_declarations) => {
                        let declaration_list: Vec<swc_css_ast::ComponentValue> =
                            list_of_declarations
                                .into_iter()
                                .map(swc_css_ast::ComponentValue::DeclarationOrAtRule)
                                .collect();

                        swc_css_ast::Stylesheet {
                            span: Default::default(),
                            rules: vec![swc_css_ast::Rule::QualifiedRule(
                                swc_css_ast::QualifiedRule {
                                    span: Default::default(),
                                    prelude: swc_css_ast::QualifiedRulePrelude::SelectorList(
                                        swc_css_ast::SelectorList {
                                            span: Default::default(),
                                            children: vec![],
                                        },
                                    ),
                                    block: swc_css_ast::SimpleBlock {
                                        span: Default::default(),
                                        name: swc_css_ast::TokenAndSpan {
                                            span: DUMMY_SP,
                                            token: swc_css_ast::Token::LBrace,
                                        },
                                        value: declaration_list,
                                    },
                                }
                                .into(),
                            )],
                        }
                    }
                    _ => return None,
                }
            }
            CssMinificationMode::MediaQueryList => {
                match swc_css_parser::parse_file::<swc_css_ast::MediaQueryList>(
                    &fm,
                    options.parser,
                    &mut errors,
                ) {
                    Ok(media_query_list) => swc_css_ast::Stylesheet {
                        span: Default::default(),
                        rules: vec![swc_css_ast::Rule::AtRule(
                            swc_css_ast::AtRule {
                                span: Default::default(),
                                name: swc_css_ast::AtRuleName::Ident(swc_css_ast::Ident {
                                    span: Default::default(),
                                    value: js_word!("media"),
                                    raw: None,
                                }),
                                prelude: Some(
                                    swc_css_ast::AtRulePrelude::MediaPrelude(media_query_list)
                                        .into(),
                                ),
                                block: Some(swc_css_ast::SimpleBlock {
                                    span: Default::default(),
                                    name: swc_css_ast::TokenAndSpan {
                                        span: DUMMY_SP,
                                        token: swc_css_ast::Token::LBrace,
                                    },
                                    // TODO make the `compress_empty` option for CSS minifier and
                                    // remove it
                                    value: vec![swc_css_ast::ComponentValue::Str(
                                        swc_css_ast::Str {
                                            span: Default::default(),
                                            value: js_word!("placeholder"),
                                            raw: None,
                                        },
                                    )],
                                }),
                            }
                            .into(),
                        )],
                    },
                    _ => return None,
                }
            }
        };

        // Avoid compress potential invalid CSS
        if !errors.is_empty() {
            return None;
        }

        swc_css_minifier::minify(&mut stylesheet, options.minifier);

        let mut minified = String::new();
        let wr = swc_css_codegen::writer::basic::BasicCssWriter::new(
            &mut minified,
            None,
            swc_css_codegen::writer::basic::BasicCssWriterConfig::default(),
        );

        options.codegen.minify = true;

        let mut gen = swc_css_codegen::CodeGenerator::new(wr, options.codegen);

        match mode {
            CssMinificationMode::Stylesheet => {
                swc_css_codegen::Emit::emit(&mut gen, &stylesheet).unwrap();
            }
            CssMinificationMode::ListOfDeclarations => {
                let swc_css_ast::Stylesheet { rules, .. } = &stylesheet;

                // Because CSS is grammar free, protect for fails
                if let Some(swc_css_ast::Rule::QualifiedRule(box swc_css_ast::QualifiedRule {
                    block,
                    ..
                })) = rules.get(0)
                {
                    swc_css_codegen::Emit::emit(&mut gen, &block).unwrap();

                    minified = minified[1..minified.len() - 1].to_string();
                } else {
                    return None;
                }
            }
            CssMinificationMode::MediaQueryList => {
                let swc_css_ast::Stylesheet { rules, .. } = &stylesheet;

                // Because CSS is grammar free, protect for fails
                if let Some(swc_css_ast::Rule::AtRule(box swc_css_ast::AtRule {
                    prelude, ..
                })) = rules.get(0)
                {
                    swc_css_codegen::Emit::emit(&mut gen, &prelude).unwrap();

                    minified = minified.trim().to_string();
                } else {
                    return None;
                }
            }
        }

        Some(minified)
    }

    fn minify_html(&self, data: String, mode: HtmlMinificationMode) -> Option<String> {
        let mut errors: Vec<_> = vec![];

        let cm = Lrc::new(SourceMap::new(FilePathMapping::empty()));
        let fm = cm.new_source_file(FileName::Anon, data);

        // Emulate content inside conditional comments like content inside the
        // `template` element
        let mut context_element = None;

        let mut document_or_document_fragment = match mode {
            HtmlMinificationMode::ConditionalComments => {
                // Emulate content inside conditional comments like content inside the
                // `template` element, because it can be used in any place in source code
                context_element = Some(Element {
                    span: Default::default(),
                    tag_name: js_word!("template"),
                    namespace: Namespace::HTML,
                    attributes: vec![],
                    children: vec![],
                    content: None,
                    is_self_closing: false,
                });

                match swc_html_parser::parse_file_as_document_fragment(
                    &fm,
                    context_element.as_ref().unwrap(),
                    DocumentMode::NoQuirks,
                    None,
                    Default::default(),
                    &mut errors,
                ) {
                    Ok(document_fragment) => HtmlRoot::DocumentFragment(document_fragment),
                    _ => return None,
                }
            }
            HtmlMinificationMode::DocumentIframeSrcdoc => {
                match swc_html_parser::parse_file_as_document(
                    &fm,
                    ParserConfig {
                        iframe_srcdoc: true,
                        ..Default::default()
                    },
                    &mut errors,
                ) {
                    Ok(document) => HtmlRoot::Document(document),
                    _ => return None,
                }
            }
        };

        // Avoid compress potential invalid CSS
        if !errors.is_empty() {
            return None;
        }

        match document_or_document_fragment {
            HtmlRoot::Document(ref mut document) => {
                minify_document(document, self.options);
            }
            HtmlRoot::DocumentFragment(ref mut document_fragment) => minify_document_fragment(
                document_fragment,
                context_element.as_ref().unwrap(),
                self.options,
            ),
        }

        let mut minified = String::new();
        let wr = swc_html_codegen::writer::basic::BasicHtmlWriter::new(
            &mut minified,
            None,
            swc_html_codegen::writer::basic::BasicHtmlWriterConfig::default(),
        );
        let mut gen = swc_html_codegen::CodeGenerator::new(
            wr,
            swc_html_codegen::CodegenConfig {
                minify: true,
                scripting_enabled: false,
                context_element: context_element.as_ref(),
                tag_omission: None,
                self_closing_void_elements: None,
                quotes: None,
            },
        );

        match document_or_document_fragment {
            HtmlRoot::Document(document) => {
                swc_html_codegen::Emit::emit(&mut gen, &document).unwrap();
            }
            HtmlRoot::DocumentFragment(document_fragment) => {
                swc_html_codegen::Emit::emit(&mut gen, &document_fragment).unwrap();
            }
        }

        Some(minified)
    }
}

impl VisitMut for Minifier<'_> {
    fn visit_mut_document(&mut self, n: &mut Document) {
        n.visit_mut_children_with(self);

        n.children
            .retain(|child| !matches!(child, Child::Comment(_) if self.options.remove_comments));
    }

    fn visit_mut_document_fragment(&mut self, n: &mut DocumentFragment) {
        n.children = self.minify_children(&mut n.children);

        n.visit_mut_children_with(self);
    }

    fn visit_mut_document_type(&mut self, n: &mut DocumentType) {
        n.visit_mut_children_with(self);

        if !self.options.force_set_html5_doctype {
            return;
        }

        n.name = Some(js_word!("html"));
        n.system_id = None;
        n.public_id = None;
    }

    fn visit_mut_child(&mut self, n: &mut Child) {
        n.visit_mut_children_with(self);

        self.current_element = None;

        if matches!(
            self.options.collapse_whitespaces,
            CollapseWhitespaces::Smart
                | CollapseWhitespaces::AdvancedConservative
                | CollapseWhitespaces::OnlyMetadata
        ) {
            match n {
                Child::Text(_) | Child::Element(_) => {
                    self.latest_element = Some(n.clone());
                }
                _ => {}
            }
        }
    }

    fn visit_mut_element(&mut self, n: &mut Element) {
        // Don't copy children to save memory
        self.current_element = Some(Element {
            span: Default::default(),
            tag_name: n.tag_name.clone(),
            namespace: n.namespace,
            attributes: n.attributes.clone(),
            children: vec![],
            content: None,
            is_self_closing: n.is_self_closing,
        });

        let old_descendant_of_pre = self.descendant_of_pre;

        if self.need_collapse_whitespace() && !old_descendant_of_pre {
            self.descendant_of_pre = get_white_space(n.namespace, &n.tag_name) == WhiteSpace::Pre;
        }

        n.children = self.minify_children(&mut n.children);

        n.visit_mut_children_with(self);

        // Remove all leading and trailing whitespaces for the `body` element
        if n.namespace == Namespace::HTML
            && n.tag_name == js_word!("body")
            && self.need_collapse_whitespace()
        {
            self.remove_leading_and_trailing_whitespaces(&mut n.children, true, true);
        }

        if self.need_collapse_whitespace() {
            self.descendant_of_pre = old_descendant_of_pre;
        }

        let mut remove_list = vec![];

        for (i, i1) in n.attributes.iter().enumerate() {
            if i1.value.is_some() {
                if self.options.remove_redundant_attributes != RemoveRedundantAttributes::None
                    && self.is_default_attribute_value(n.namespace, &n.tag_name, i1)
                {
                    remove_list.push(i);

                    continue;
                }

                if self.options.remove_empty_attributes {
                    let value = i1.value.as_ref().unwrap();

                    if (matches!(i1.name, js_word!("id")) && value.is_empty())
                        || (matches!(i1.name, js_word!("class") | js_word!("style"))
                            && value.is_empty())
                        || self.is_event_handler_attribute(i1) && value.is_empty()
                    {
                        remove_list.push(i);

                        continue;
                    }
                }
            }

            for (j, j1) in n.attributes.iter().enumerate() {
                if i < j && i1.name == j1.name {
                    remove_list.push(j);
                }
            }
        }

        // Fast path. We don't face real duplicates in most cases.
        if !remove_list.is_empty() {
            let new = take(&mut n.attributes)
                .into_iter()
                .enumerate()
                .filter_map(|(idx, value)| {
                    if remove_list.contains(&idx) {
                        None
                    } else {
                        Some(value)
                    }
                })
                .collect::<Vec<_>>();

            n.attributes = new;
        }

        if let Some(attribute_name_counter) = &self.attribute_name_counter {
            n.attributes.sort_by(|a, b| {
                let ordeing = attribute_name_counter
                    .get(&b.name)
                    .cmp(&attribute_name_counter.get(&a.name));

                match ordeing {
                    Ordering::Equal => b.name.cmp(&a.name),
                    _ => ordeing,
                }
            });
        }
    }

    fn visit_mut_attribute(&mut self, n: &mut Attribute) {
        n.visit_mut_children_with(self);

        if let Some(value) = &n.value {
            let current_element = match &self.current_element {
                Some(current_element) => current_element,
                _ => return,
            };

            if value.is_empty() {
                if (self.options.collapse_boolean_attributes
                    && self.is_boolean_attribute(current_element, n))
                    || (self.options.normalize_attributes
                        && self.is_crossorigin_attribute(current_element, n)
                        && value.is_empty())
                {
                    n.value = None;
                }

                return;
            }

            match (
                current_element.namespace,
                &current_element.tag_name,
                &n.name,
            ) {
                (Namespace::HTML, &js_word!("iframe"), &js_word!("srcdoc")) => {
                    if let Some(minified) = self.minify_html(
                        value.to_string(),
                        HtmlMinificationMode::DocumentIframeSrcdoc,
                    ) {
                        n.value = Some(minified.into());
                    };
                }
                (
                    Namespace::HTML | Namespace::SVG,
                    &js_word!("style")
                    | &js_word!("link")
                    | &js_word!("script")
                    | &js_word!("input"),
                    &js_word!("type"),
                ) if self.options.normalize_attributes => {
                    n.value = Some(value.trim().to_ascii_lowercase().into());
                }
                _ if self.options.normalize_attributes
                    && self.is_crossorigin_attribute(current_element, n)
                    && value.to_ascii_lowercase() == js_word!("anonymous") =>
                {
                    n.value = None;
                }
                _ if self.options.collapse_boolean_attributes
                    && self.is_boolean_attribute(current_element, n) =>
                {
                    n.value = None;
                }
                _ if self.is_event_handler_attribute(n) => {
                    let mut value = value.to_string();

                    if self.options.normalize_attributes {
                        value = value.trim().into();

                        if value.trim().to_lowercase().starts_with("javascript:") {
                            value = value.chars().skip(11).collect();
                        }
                    }

                    if self.need_minify_js() {
                        if let Some(minified) = self.minify_js(value, false, true) {
                            n.value = Some(minified.into());
                        };
                    } else {
                        n.value = Some(value.into());
                    }
                }
                _ if self.options.normalize_attributes
                    && current_element.namespace == Namespace::HTML
                    && n.name == js_word!("contenteditable")
                    && n.value == Some(js_word!("true")) =>
                {
                    n.value = Some(js_word!(""));
                }
                _ if self.options.normalize_attributes
                    && self.is_semicolon_separated_attribute(current_element, n) =>
                {
                    n.value = Some(
                        value
                            .split(';')
                            .map(|value| self.collapse_whitespace(value.trim()))
                            .collect::<Vec<_>>()
                            .join(";")
                            .into(),
                    );
                }
                _ if self.options.normalize_attributes
                    && n.name == js_word!("content")
                    && self.element_has_attribute_with_value(
                        current_element,
                        &js_word!("http-equiv"),
                        &[js_word!("content-security-policy")],
                    ) =>
                {
                    let mut new_values = vec![];

                    for value in value.trim().split(';') {
                        new_values.push(
                            value
                                .trim()
                                .split(' ')
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                    }

                    let mut value = new_values.join(";");

                    if value.ends_with(';') {
                        value.pop();
                    }

                    n.value = Some(value.into());
                }
                _ if self.options.sort_space_separated_attribute_values
                    && self.is_attribute_value_unordered_set(current_element, n) =>
                {
                    let mut values = value.split_whitespace().collect::<Vec<_>>();

                    values.sort_unstable();

                    n.value = Some(values.join(" ").into());
                }
                _ if self.options.normalize_attributes
                    && self.is_space_separated_attribute(current_element, n) =>
                {
                    n.value = Some(
                        value
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            .into(),
                    );
                }
                _ if self.is_comma_separated_attribute(current_element, n) => {
                    let mut value = value.to_string();

                    if self.options.normalize_attributes {
                        value = value
                            .split(',')
                            .map(|value| {
                                if matches!(n.name, js_word!("sizes") | js_word!("imagesizes")) {
                                    let trimmed = value.trim();

                                    match self.minify_sizes(trimmed) {
                                        Some(minified) => minified,
                                        _ => trimmed.to_string(),
                                    }
                                } else if matches!(n.name, js_word!("points")) {
                                    self.collapse_whitespace(value.trim())
                                } else if matches!(n.name, js_word!("exportparts")) {
                                    value.chars().filter(|c| !c.is_whitespace()).collect()
                                } else {
                                    value.trim().to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(",");
                    }

                    if self.need_minify_css() && n.name == js_word!("media") && !value.is_empty() {
                        if let Some(minified) =
                            self.minify_css(value, CssMinificationMode::MediaQueryList)
                        {
                            n.value = Some(minified.into());
                        }
                    } else {
                        n.value = Some(value.into());
                    }
                }
                _ if self.is_trimable_separated_attribute(current_element, n) => {
                    let mut value = value.to_string();

                    let fallback = |n: &mut Attribute| {
                        if self.options.normalize_attributes {
                            n.value = Some(value.trim().into());
                        }
                    };

                    if self.need_minify_css() && n.name == js_word!("style") && !value.is_empty() {
                        let value = value.trim();

                        if let Some(minified) = self
                            .minify_css(value.to_string(), CssMinificationMode::ListOfDeclarations)
                        {
                            n.value = Some(minified.into());
                        } else {
                            fallback(n);
                        }
                    } else if self.need_minify_js()
                        && self.is_javascript_url_element(current_element)
                    {
                        if value.trim().to_lowercase().starts_with("javascript:") {
                            value = value.trim().chars().skip(11).collect();

                            if let Some(minified) = self.minify_js(value, false, true) {
                                let mut with_javascript =
                                    String::with_capacity(11 + minified.len());

                                with_javascript.push_str("javascript:");
                                with_javascript.push_str(&minified);

                                n.value = Some(with_javascript.into());
                            }
                        } else {
                            fallback(n);
                        }
                    } else {
                        fallback(n);
                    }
                }
                _ if self.options.minify_additional_attributes.is_some() => {
                    match self.is_additional_minifier_attribute(&n.name) {
                        Some(MinifierType::JsScript) if self.need_minify_js() => {
                            if let Some(minified) = self.minify_js(value.to_string(), false, true) {
                                n.value = Some(minified.into());
                            }
                        }
                        Some(MinifierType::JsModule) if self.need_minify_js() => {
                            if let Some(minified) = self.minify_js(value.to_string(), true, true) {
                                n.value = Some(minified.into());
                            }
                        }
                        Some(MinifierType::Json) if self.need_minify_json() => {
                            if let Some(minified) = self.minify_json(value.to_string()) {
                                n.value = Some(minified.into());
                            }
                        }
                        Some(MinifierType::Css) if self.need_minify_css() => {
                            if let Some(minified) = self.minify_css(
                                value.to_string(),
                                CssMinificationMode::ListOfDeclarations,
                            ) {
                                n.value = Some(minified.into());
                            }
                        }
                        Some(MinifierType::Html) => {
                            if let Some(minified) = self.minify_html(
                                value.to_string(),
                                HtmlMinificationMode::DocumentIframeSrcdoc,
                            ) {
                                n.value = Some(minified.into());
                            };
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_mut_text(&mut self, n: &mut Text) {
        n.visit_mut_children_with(self);

        if n.data.len() == 0 {
            return;
        }

        let mut text_type = None;

        if let Some(current_element) = &self.current_element {
            match current_element.tag_name {
                js_word!("script")
                    if (self.need_minify_json() || self.need_minify_js())
                        && matches!(
                            current_element.namespace,
                            Namespace::HTML | Namespace::SVG
                        )
                        && !current_element
                            .attributes
                            .iter()
                            .any(|attribute| matches!(attribute.name, js_word!("src"))) =>
                {
                    let type_attribute_value: Option<JsWord> = self
                        .get_attribute_value(&current_element.attributes, js_word!("type"))
                        .map(|v| v.to_ascii_lowercase().trim().into());

                    match type_attribute_value {
                        Some(js_word!("module")) if self.need_minify_js() => {
                            text_type = Some(MinifierType::JsModule);
                        }
                        Some(value)
                            if self.need_minify_js() && self.is_type_text_javascript(&value) =>
                        {
                            text_type = Some(MinifierType::JsScript);
                        }
                        None if self.need_minify_js() => {
                            text_type = Some(MinifierType::JsScript);
                        }
                        Some(
                            js_word!("application/json")
                            | js_word!("application/ld+json")
                            | js_word!("importmap")
                            | js_word!("speculationrules"),
                        ) if self.need_minify_json() => {
                            text_type = Some(MinifierType::Json);
                        }
                        Some(script_type)
                            if self.options.minify_additional_scripts_content.is_some() =>
                        {
                            if let Some(minifier_type) =
                                self.is_additional_scripts_content(&script_type)
                            {
                                text_type = Some(minifier_type);
                            }
                        }
                        _ => {}
                    }
                }
                js_word!("style")
                    if self.need_minify_css()
                        && matches!(
                            current_element.namespace,
                            Namespace::HTML | Namespace::SVG
                        ) =>
                {
                    let mut type_attribute_value = None;

                    for attribute in &current_element.attributes {
                        if attribute.name == js_word!("type") && attribute.value.is_some() {
                            type_attribute_value = Some(
                                attribute
                                    .value
                                    .as_ref()
                                    .unwrap()
                                    .trim()
                                    .to_ascii_lowercase(),
                            );

                            break;
                        }
                    }

                    if type_attribute_value.is_none()
                        || type_attribute_value == Some("text/css".into())
                    {
                        text_type = Some(MinifierType::Css)
                    }
                }
                js_word!("title") if current_element.namespace == Namespace::HTML => {
                    n.data = self.collapse_whitespace(&n.data).trim().into();
                }
                _ => {}
            }
        }

        match text_type {
            Some(MinifierType::JsScript) => {
                let minified = match self.minify_js(n.data.to_string(), false, false) {
                    Some(minified) => minified,
                    None => return,
                };

                n.data = minified.into();
            }
            Some(MinifierType::JsModule) => {
                let minified = match self.minify_js(n.data.to_string(), true, false) {
                    Some(minified) => minified,
                    None => return,
                };

                n.data = minified.into();
            }
            Some(MinifierType::Json) => {
                let minified = match self.minify_json(n.data.to_string()) {
                    Some(minified) => minified,
                    None => return,
                };

                n.data = minified.into();
            }
            Some(MinifierType::Css) => {
                let minified =
                    match self.minify_css(n.data.to_string(), CssMinificationMode::Stylesheet) {
                        Some(minified) => minified,
                        None => return,
                    };

                n.data = minified.into();
            }
            Some(MinifierType::Html) => {
                let minified = match self.minify_html(
                    n.data.to_string(),
                    HtmlMinificationMode::ConditionalComments,
                ) {
                    Some(minified) => minified,
                    None => return,
                };

                n.data = minified.into();
            }
            _ => {}
        }
    }

    fn visit_mut_comment(&mut self, n: &mut Comment) {
        n.visit_mut_children_with(self);

        if !self.options.minify_conditional_comments {
            return;
        }

        if self.is_conditional_comment(&n.data) && n.data.len() > 0 {
            let start_pos = match n.data.find("]>") {
                Some(start_pos) => start_pos,
                _ => return,
            };
            let end_pos = match n.data.find("<![") {
                Some(end_pos) => end_pos,
                _ => return,
            };

            let html = n
                .data
                .chars()
                .skip(start_pos)
                .take(end_pos - start_pos)
                .collect();

            let minified = match self.minify_html(html, HtmlMinificationMode::ConditionalComments) {
                Some(minified) => minified,
                _ => return,
            };
            let before: String = n.data.chars().take(start_pos).collect();
            let after: String = n.data.chars().skip(end_pos).take(n.data.len()).collect();
            let mut data = String::with_capacity(n.data.len());

            data.push_str(&before);
            data.push_str(&minified);
            data.push_str(&after);

            n.data = data.into();
        }
    }
}

struct AttributeNameCounter {
    tree: AHashMap<JsWord, usize>,
}

impl VisitMut for AttributeNameCounter {
    fn visit_mut_attribute(&mut self, n: &mut Attribute) {
        n.visit_mut_children_with(self);

        *self.tree.entry(n.name.clone()).or_insert(0) += 1;
    }
}

fn create_minifier<'a>(
    context_element: Option<&Element>,
    options: &'a MinifyOptions,
) -> Minifier<'a> {
    let mut current_element = None;
    let mut is_pre = false;

    if let Some(context_element) = context_element {
        current_element = Some(context_element.clone());
        is_pre = get_white_space(context_element.namespace, &context_element.tag_name)
            == WhiteSpace::Pre;
    }

    Minifier {
        options,

        current_element,
        latest_element: None,
        descendant_of_pre: is_pre,
        attribute_name_counter: None,
    }
}

pub fn minify_document(document: &mut Document, options: &MinifyOptions) {
    let mut minifier = create_minifier(None, options);

    if options.sort_attributes {
        let mut attribute_name_counter = AttributeNameCounter {
            tree: Default::default(),
        };

        document.visit_mut_with(&mut attribute_name_counter);

        minifier.attribute_name_counter = Some(attribute_name_counter.tree);
    }

    document.visit_mut_with(&mut minifier);
}

pub fn minify_document_fragment(
    document_fragment: &mut DocumentFragment,
    context_element: &Element,
    options: &MinifyOptions,
) {
    let mut minifier = create_minifier(Some(context_element), options);

    if options.sort_attributes {
        let mut attribute_name_counter = AttributeNameCounter {
            tree: Default::default(),
        };

        document_fragment.visit_mut_with(&mut attribute_name_counter);

        minifier.attribute_name_counter = Some(attribute_name_counter.tree);
    }

    document_fragment.visit_mut_with(&mut minifier);
}
