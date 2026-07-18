use html5ever::tokenizer::{
    BufferQueue, CharacterTokens, EndTag, StartTag, TagToken, Token, TokenSink,
    TokenSinkResult, Tokenizer,
};
use html5ever::tendril::StrTendril;
use std::cell::RefCell;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlNode {
    Element(HtmlElement),
    Text(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlElement {
    pub name: String,
    pub attrs: BTreeMap<String, String>,
    pub children: Vec<HtmlNode>,
}

#[derive(Debug, Clone)]
enum SimpleToken {
    Start {
        name: String,
        attrs: BTreeMap<String, String>,
        self_closing: bool,
    },
    End(String),
    Text(String),
}

#[derive(Default)]
struct Collector(RefCell<Vec<SimpleToken>>);

impl TokenSink for Collector {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        match token {
            CharacterTokens(text) => {
                if !text.is_empty() {
                    self.0.borrow_mut().push(SimpleToken::Text(text.to_string()));
                }
            }
            TagToken(tag) => match tag.kind {
                StartTag => {
                    let attrs = tag
                        .attrs
                        .into_iter()
                        .map(|attr| (attr.name.local.to_string(), attr.value.to_string()))
                        .collect();
                    self.0.borrow_mut().push(SimpleToken::Start {
                        name: tag.name.to_string(),
                        attrs,
                        self_closing: tag.self_closing,
                    });
                }
                EndTag => self
                    .0
                    .borrow_mut()
                    .push(SimpleToken::End(tag.name.to_string())),
            },
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

pub fn parse_fragment(source: &str) -> HtmlElement {
    let input = BufferQueue::default();
    input.push_back(StrTendril::from(source));
    let tokenizer = Tokenizer::new(Collector::default(), Default::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    let tokens = tokenizer.sink.0.into_inner();

    let mut stack = vec![HtmlElement {
        name: "root".to_string(),
        attrs: BTreeMap::new(),
        children: Vec::new(),
    }];
    for token in tokens {
        match token {
            SimpleToken::Text(text) => append_text(stack.last_mut().expect("root exists"), text),
            SimpleToken::Start {
                name,
                attrs,
                self_closing,
            } => {
                let element = HtmlElement {
                    name,
                    attrs,
                    children: Vec::new(),
                };
                if self_closing || is_void_element(&element.name) {
                    stack
                        .last_mut()
                        .expect("root exists")
                        .children
                        .push(HtmlNode::Element(element));
                } else {
                    stack.push(element);
                }
            }
            SimpleToken::End(name) => close_through(&mut stack, &name),
        }
    }
    while stack.len() > 1 {
        close_top(&mut stack);
    }
    stack.pop().expect("root exists")
}

fn append_text(parent: &mut HtmlElement, text: String) {
    if let Some(HtmlNode::Text(previous)) = parent.children.last_mut() {
        previous.push_str(&text);
    } else {
        parent.children.push(HtmlNode::Text(text));
    }
}

fn close_through(stack: &mut Vec<HtmlElement>, name: &str) {
    let Some(position) = stack.iter().rposition(|element| element.name == name) else {
        return;
    };
    while stack.len() > position {
        close_top(stack);
    }
}

fn close_top(stack: &mut Vec<HtmlElement>) {
    if stack.len() <= 1 {
        return;
    }
    let element = stack.pop().expect("non-root element exists");
    stack
        .last_mut()
        .expect("root exists")
        .children
        .push(HtmlNode::Element(element));
}

fn is_void_element(name: &str) -> bool {
    matches!(name, "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr")
}

impl HtmlElement {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    pub fn has_class(&self, class: &str) -> bool {
        self.attr("class")
            .is_some_and(|value| value.split_ascii_whitespace().any(|item| item == class))
    }

    pub fn text(&self) -> String {
        let mut output = String::new();
        collect_text(&self.children, &mut output, &[]);
        output
    }

    pub fn text_excluding_classes(&self, excluded: &[&str]) -> String {
        let mut output = String::new();
        collect_text(&self.children, &mut output, excluded);
        output
    }

    pub fn first_by_class(&self, class: &str) -> Option<&HtmlElement> {
        if self.has_class(class) {
            return Some(self);
        }
        self.children.iter().find_map(|child| match child {
            HtmlNode::Element(element) => element.first_by_class(class),
            HtmlNode::Text(_) => None,
        })
    }

    pub fn all_by_class<'a>(&'a self, class: &str, output: &mut Vec<&'a HtmlElement>) {
        if self.has_class(class) {
            output.push(self);
        }
        for child in &self.children {
            if let HtmlNode::Element(element) = child {
                element.all_by_class(class, output);
            }
        }
    }

    pub fn all_by_name<'a>(&'a self, name: &str, output: &mut Vec<&'a HtmlElement>) {
        if self.name == name {
            output.push(self);
        }
        for child in &self.children {
            if let HtmlNode::Element(element) = child {
                element.all_by_name(name, output);
            }
        }
    }

    pub fn all_elements<'a>(&'a self, output: &mut Vec<&'a HtmlElement>) {
        for child in &self.children {
            if let HtmlNode::Element(element) = child {
                output.push(element);
                element.all_elements(output);
            }
        }
    }
}

fn collect_text(nodes: &[HtmlNode], output: &mut String, excluded: &[&str]) {
    for node in nodes {
        match node {
            HtmlNode::Text(text) => output.push_str(text),
            HtmlNode::Element(element) => {
                if excluded.iter().any(|class| element.has_class(class)) {
                    continue;
                }
                if element.name == "br" && !output.ends_with('\n') {
                    output.push('\n');
                }
                collect_text(&element.children, output, excluded);
                if matches!(element.name.as_str(), "p" | "div" | "section" | "li" | "dd")
                    && !output.ends_with('\n')
                {
                    output.push('\n');
                }
            }
        }
    }
}
