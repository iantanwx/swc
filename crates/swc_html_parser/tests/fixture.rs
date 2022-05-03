#![allow(clippy::needless_update)]

use std::{fs, mem::take, path::PathBuf};

use serde_json::Value;
use swc_atoms::JsWord;
use swc_common::{
    collections::AHashSet,
    errors::Handler,
    input::{SourceFileInput, StringInput},
    BytePos, Spanned,
};
use swc_html_ast::*;
use swc_html_parser::{
    error::ErrorKind,
    lexer::{Lexer, State},
    parser::{input::ParserInput, PResult, Parser, ParserConfig},
};
use swc_html_visit::{Visit, VisitWith};
use testing::NormalizedOutput;

fn test_pass(input: PathBuf, config: ParserConfig) {
    testing::run_test2(false, |cm, handler| {
        let json_path = input.parent().unwrap().join("output.json");
        let fm = cm.load_file(&input).unwrap();
        let lexer = Lexer::new(SourceFileInput::from(&*fm), config);
        let mut parser = Parser::new(lexer, config);
        let document: PResult<Document> = parser.parse_document();
        let errors = parser.take_errors();

        for err in &errors {
            err.to_diagnostics(&handler).emit();
        }

        if !errors.is_empty() {
            return Err(());
        }

        match document {
            Ok(document) => {
                let actual_json = serde_json::to_string_pretty(&document)
                    .map(NormalizedOutput::from)
                    .expect("failed to serialize document");

                actual_json.compare_to_file(&json_path).unwrap();

                Ok(())
            }
            Err(err) => {
                let mut d = err.to_diagnostics(&handler);

                d.note(&format!("current token = {}", parser.dump_cur()));
                d.emit();

                Err(())
            }
        }
    })
    .unwrap();
}

fn test_recovery(input: PathBuf, config: ParserConfig) {
    let stderr_path = input.parent().unwrap().join("output.stderr");
    let mut recovered = false;

    let stderr = testing::run_test2(false, |cm, handler| {
        // Type annotation
        if false {
            return Ok(());
        }

        let json_path = input.parent().unwrap().join("output.json");
        let fm = cm.load_file(&input).unwrap();
        let lexer = Lexer::new(SourceFileInput::from(&*fm), config);
        let mut parser = Parser::new(lexer, config);
        let document: PResult<Document> = parser.parse_document();
        let errors = parser.take_errors();

        for err in &errors {
            err.to_diagnostics(&handler).emit();
        }

        if !errors.is_empty() {
            recovered = true;
        }

        match document {
            Ok(document) => {
                let actual_json = serde_json::to_string_pretty(&document)
                    .map(NormalizedOutput::from)
                    .expect("failed to serialize document");

                actual_json.compare_to_file(&json_path).unwrap();

                Err(())
            }
            Err(err) => {
                let mut d = err.to_diagnostics(&handler);

                d.note(&format!("current token = {}", parser.dump_cur()));
                d.emit();

                Err(())
            }
        }
    })
    .unwrap_err();

    if !recovered {
        panic!(
            "Parser should emit errors (recover mode), but parser parsed everything successfully \
             {}",
            stderr
        );
    }

    stderr.compare_to_file(&stderr_path).unwrap();
}

struct SpanVisualizer<'a> {
    handler: &'a Handler,
}

macro_rules! mtd {
    ($T:ty,$name:ident) => {
        fn $name(&mut self, n: &$T) {
            self.handler
                .struct_span_err(n.span(), stringify!($T))
                .emit();

            n.visit_children_with(self);
        }
    };
}

impl Visit for SpanVisualizer<'_> {
    mtd!(Document, visit_document);

    mtd!(DocumentFragment, visit_document_fragment);

    mtd!(Child, visit_child);

    mtd!(DocumentType, visit_document_type);

    mtd!(Element, visit_element);

    mtd!(Attribute, visit_attribute);

    mtd!(Text, visit_text);

    mtd!(Comment, visit_comment);

    fn visit_token_and_span(&mut self, n: &TokenAndSpan) {
        self.handler
            .struct_span_err(n.span, &format!("{:?}", n.token))
            .emit();
    }
}

fn test_span_visualizer(input: PathBuf, config: ParserConfig) {
    let dir = input.parent().unwrap().to_path_buf();

    let output = testing::run_test2(false, |cm, handler| {
        // Type annotation
        if false {
            return Ok(());
        }

        let fm = cm.load_file(&input).unwrap();
        let lexer = Lexer::new(SourceFileInput::from(&*fm), config);
        let mut parser = Parser::new(lexer, config);

        let document: PResult<Document> = parser.parse_document();

        match document {
            Ok(document) => {
                document.visit_with(&mut SpanVisualizer { handler: &handler });

                Err(())
            }
            Err(err) => {
                let mut d = err.to_diagnostics(&handler);

                d.note(&format!("current token = {}", parser.dump_cur()));
                d.emit();

                panic!();
            }
        }
    })
    .unwrap_err();

    output
        .compare_to_file(&dir.join("span.rust-debug"))
        .unwrap();
}

#[testing::fixture("tests/fixture/**/input.html")]
fn pass(input: PathBuf) {
    test_pass(
        input,
        ParserConfig {
            ..Default::default()
        },
    )
}

#[testing::fixture("tests/recovery/**/input.html")]
fn recovery(input: PathBuf) {
    test_recovery(
        input,
        ParserConfig {
            ..Default::default()
        },
    )
}

#[testing::fixture("tests/fixture/**/input.html")]
#[testing::fixture("tests/recovery/**/input.html")]
fn span_visualizer(input: PathBuf) {
    test_span_visualizer(
        input,
        ParserConfig {
            ..Default::default()
        },
    )
}

fn unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();

    loop {
        match it.next() {
            None => return Some(out),
            Some('\\') => {
                if it.peek() != Some(&'u') {
                    panic!("can't understand escape");
                }

                let hex: String = it.by_ref().take(4).collect();

                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    // TODO fix me surrogate paris
                    // Some of the tests use lone surrogates, but we have no
                    // way to represent them in the UTF-8 input to our parser.
                    // Since these can only come from script, we will catch
                    // them there.
                    None => return None,
                    Some(c) => out.push(c),
                }
            }
            Some(c) => out.push(c),
        }
    }
}

// TODO we need to enable `preserve_order` for serde, but we can't https://github.com/tkaitchuck/aHash/issues/95, so we sort attributes
#[testing::fixture("tests/html5lib-tests/tokenizer/**/*.test")]
fn html5lib_test_tokenizer(input: PathBuf) {
    let filename = input.to_str().expect("failed to parse path");
    let contents = fs::read_to_string(filename).expect("Something went wrong reading the file");
    let obj: Value = serde_json::from_str(&contents)
        .ok()
        .expect("json parse error");
    let tests = match obj.get(&"tests".to_string()) {
        Some(&Value::Array(ref tests)) => tests,
        _ => {
            return ();
        }
    };

    for test in tests.iter() {
        let description = test
            .get("description")
            .expect("failed to get input in test");

        let states = if let Some(initial_states) = test.get("initialStates") {
            let mut states = vec![];
            let json_states: Vec<String> = serde_json::from_value(initial_states.clone())
                .expect("failed to get input in test");

            for json_state in json_states {
                match &*json_state {
                    "Data state" => {
                        states.push(State::Data);
                    }
                    "PLAINTEXT state" => {
                        states.push(State::PlainText);
                    }
                    "RCDATA state" => {
                        states.push(State::Rcdata);
                    }
                    "RAWTEXT state" => {
                        states.push(State::Rawtext);
                    }
                    "Script data state" => {
                        states.push(State::ScriptData);
                    }
                    "CDATA section state" => {
                        states.push(State::CdataSection);
                    }
                    _ => {
                        unreachable!()
                    }
                }
            }

            states
        } else {
            vec![State::Data]
        };

        for state in states.iter() {
            eprintln!("==== ==== Description ==== ====\n{}\n", description);

            let json_input = test["input"].clone();
            let mut input: String =
                serde_json::from_value(json_input).expect("failed to get input in test");

            let need_double_escaped = test.get("doubleEscaped").is_some();

            if need_double_escaped {
                input = match unescape(&input) {
                    Some(unescaped) => unescaped,
                    _ => {
                        continue;
                    }
                };
            }

            eprintln!("==== ==== Input ==== ====\n{}\n", input);

            let json_output = test["output"].clone();
            let output = json_output.to_string();

            eprintln!("==== ==== Output ==== ====\n{}\n", output);

            // TODO keep `\r` in raw when we implement them in AST or implement an option
            let lexer_input = input.replace("\r\n", "\n").replace('\r', "\n");
            let lexer_str_input =
                StringInput::new(&lexer_input, BytePos(0), BytePos(input.len() as u32));
            let mut lexer = Lexer::new(
                lexer_str_input,
                ParserConfig {
                    ..Default::default()
                },
            );

            lexer.set_input_state(state.clone());

            if let Some(last_start_tag) = test.get("lastStartTag") {
                let last_start_tag: String = serde_json::from_value(last_start_tag.clone())
                    .expect("failed to get lastStartTag in test");

                lexer.last_start_tag_token = Some(Token::StartTag {
                    tag_name: last_start_tag.into(),
                    raw_tag_name: None,
                    self_closing: false,
                    attributes: vec![],
                });
            }

            let mut actual_tokens = vec![];

            loop {
                match lexer.next() {
                    Ok(TokenAndSpan { token, .. }) => {
                        let mut new_token = token.clone();

                        match new_token {
                            Token::Doctype {
                                ref mut raw_keyword,
                                ref mut raw_name,
                                ref mut public_quote,
                                ref mut raw_public_keyword,
                                ref mut system_quote,
                                ref mut raw_system_keyword,
                                ..
                            } => {
                                *raw_keyword = None;
                                *raw_name = None;
                                *public_quote = None;
                                *raw_public_keyword = None;
                                *system_quote = None;
                                *raw_system_keyword = None;
                            }
                            Token::StartTag {
                                ref mut raw_tag_name,
                                ref mut attributes,
                                ..
                            } => {
                                *raw_tag_name = None;

                                let mut new_attributes = vec![];
                                let mut already_seen: AHashSet<JsWord> = Default::default();

                                for mut attribute in take(attributes) {
                                    if already_seen.contains(&attribute.name) {
                                        continue;
                                    }

                                    already_seen.insert(attribute.name.clone());

                                    if attribute.value.is_none() {
                                        attribute.value = Some("".into());
                                    }

                                    attribute.raw_name = None;
                                    attribute.raw_value = None;

                                    new_attributes.push(attribute);
                                }

                                new_attributes.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());

                                *attributes = new_attributes;
                            }
                            Token::EndTag {
                                ref mut raw_tag_name,
                                ref mut attributes,
                                ref mut self_closing,
                                ..
                            } => {
                                *raw_tag_name = None;
                                *self_closing = false;
                                *attributes = vec![];
                            }
                            Token::Character { ref mut raw, .. } => {
                                *raw = None;
                            }
                            _ => {}
                        }

                        actual_tokens.push(new_token);
                    }
                    Err(error) => match error.kind() {
                        ErrorKind::Eof => {
                            break;
                        }
                        _ => {
                            unreachable!();
                        }
                    },
                };
            }

            let mut expected_tokens: Vec<Token> = vec![];

            for output_tokens in json_output.as_array() {
                for output_token in output_tokens {
                    match output_token {
                        Value::Array(token_parts) => {
                            let tokens = match &*token_parts[0].as_str().expect("failed") {
                                "DOCTYPE" => {
                                    let name: Option<String> =
                                        serde_json::from_value(token_parts[1].clone())
                                            .expect("failed to deserialize");
                                    let public_id: Option<String> =
                                        serde_json::from_value(token_parts[2].clone())
                                            .expect("failed to deserialize");
                                    let system_id: Option<String> =
                                        serde_json::from_value(token_parts[3].clone())
                                            .expect("failed to deserialize");
                                    let correctness: bool =
                                        serde_json::from_value(token_parts[4].clone())
                                            .expect("failed to deserialize");

                                    vec![Token::Doctype {
                                        raw_keyword: None,
                                        name: name.map(|v| v.into()),
                                        raw_name: None,
                                        force_quirks: !correctness,
                                        raw_public_keyword: None,
                                        public_quote: None,
                                        public_id: public_id.map(|v| v.into()),
                                        raw_system_keyword: None,
                                        system_quote: None,
                                        system_id: system_id.map(|v| v.into()),
                                    }]
                                }
                                "StartTag" => {
                                    let tag_name: String =
                                        serde_json::from_value(token_parts[1].clone())
                                            .expect("failed to deserialize");
                                    let mut attributes = vec![];

                                    if let Some(json_attributes) = token_parts.get(2).clone() {
                                        let obj_attributes: Value =
                                            serde_json::from_value(json_attributes.clone())
                                                .expect("failed to deserialize");

                                        match obj_attributes {
                                            Value::Object(obj) => {
                                                for key in obj.keys() {
                                                    let json_value = obj.get(key).expect(
                                                        "failed to get value for  attribute",
                                                    );
                                                    let value: Option<String> =
                                                        serde_json::from_value(json_value.clone())
                                                            .expect("failed to deserialize");

                                                    attributes.push(AttributeToken {
                                                        name: key.clone().into(),
                                                        raw_name: None,
                                                        value: value.map(|v| v.into()),
                                                        raw_value: None,
                                                    })
                                                }
                                            }
                                            _ => {
                                                unreachable!();
                                            }
                                        }
                                    }

                                    let mut self_closing = false;

                                    if let Some(json_self_closing) = token_parts.get(3).clone() {
                                        let value: bool =
                                            serde_json::from_value(json_self_closing.clone())
                                                .expect("failed to deserialize");

                                        self_closing = value;
                                    }

                                    attributes.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());

                                    vec![Token::StartTag {
                                        tag_name: tag_name.into(),
                                        raw_tag_name: None,
                                        self_closing,
                                        attributes,
                                    }]
                                }
                                "EndTag" => {
                                    let tag_name: String =
                                        serde_json::from_value(token_parts[1].clone())
                                            .expect("failed to deserialize");

                                    vec![Token::EndTag {
                                        tag_name: tag_name.into(),
                                        raw_tag_name: None,
                                        self_closing: false,
                                        attributes: vec![],
                                    }]
                                }
                                "Character" => {
                                    let mut data: String =
                                        serde_json::from_value(token_parts[1].clone())
                                            .expect("failed to deserialize");

                                    if need_double_escaped {
                                        data = match unescape(&data) {
                                            Some(v) => v,
                                            _ => {
                                                continue;
                                            }
                                        };
                                    }

                                    let mut tokens = vec![];

                                    for c in data.chars() {
                                        tokens.push(Token::Character {
                                            value: c,
                                            raw: None,
                                        })
                                    }

                                    tokens
                                }
                                "Comment" => {
                                    let mut data: String =
                                        serde_json::from_value(token_parts[1].clone())
                                            .expect("failed to deserialize");

                                    if need_double_escaped {
                                        data = match unescape(&data) {
                                            Some(v) => v,
                                            _ => {
                                                continue;
                                            }
                                        };
                                    }

                                    vec![Token::Comment { data: data.into() }]
                                }
                                _ => {
                                    unreachable!("unknown token {}", token_parts[0])
                                }
                            };

                            expected_tokens.extend(tokens);
                        }
                        _ => {
                            unreachable!();
                        }
                    }
                }
            }

            let actual =
                serde_json::to_string(&actual_tokens).expect("failed to serialize actual tokens");
            let expected = serde_json::to_string(&expected_tokens)
                .expect("failed to serialize expected tokens");

            if let Some(json_errors) = test.get("errors") {
                let expected_errors = json_errors.as_array().expect("failed to deserialize error");
                let actual_errors = lexer.take_errors();

                eprintln!("==== ==== Errors ==== ====\n{:?}\n", actual_errors);

                assert_eq!(actual_errors.len(), expected_errors.len());

                for expected_error in expected_errors.iter() {
                    let obj_expected_code =
                        expected_error.as_object().expect("failed to get error");
                    let expected_code = match obj_expected_code.get("code") {
                        Some(expected_code) => match expected_code.as_str() {
                            Some("eof-in-doctype") => ErrorKind::EofInDoctype,
                            Some("eof-in-comment") => ErrorKind::EofInComment,
                            Some("eof-in-cdata") => ErrorKind::EofInCdata,
                            Some("eof-in-tag") => ErrorKind::EofInTag,
                            Some("eof-before-tag-name") => ErrorKind::EofBeforeTagName,
                            Some("eof-in-script-html-comment-like-text") => {
                                ErrorKind::EofInScriptHtmlCommentLikeText
                            }
                            Some("unknown-named-character-reference") => {
                                ErrorKind::UnknownNamedCharacterReference
                            }
                            Some("incorrectly-opened-comment") => {
                                ErrorKind::IncorrectlyOpenedComment
                            }
                            Some("abrupt-closing-of-empty-comment") => {
                                ErrorKind::AbruptClosingOfEmptyComment
                            }
                            Some("abrupt-doctype-public-identifier") => {
                                ErrorKind::AbruptDoctypePublicIdentifier
                            }
                            Some("abrupt-doctype-system-identifier") => {
                                ErrorKind::AbruptDoctypeSystemIdentifier
                            }
                            Some("absence-of-digits-in-numeric-character-reference") => {
                                ErrorKind::AbsenceOfDigitsInNumericCharacterReference
                            }
                            Some("surrogate-character-reference") => {
                                ErrorKind::SurrogateCharacterReference
                            }
                            Some("nested-comment") => ErrorKind::NestedComment,
                            Some("end-tag-with-trailing-solidus") => {
                                ErrorKind::EndTagWithTrailingSolidus
                            }
                            Some("null-character-reference") => ErrorKind::NullCharacterReference,
                            Some("cdata-in-html-content") => ErrorKind::CdataInHtmlContent,
                            Some("character-reference-outside-unicode-range") => {
                                ErrorKind::CharacterReferenceOutsideUnicodeRange
                            }
                            Some("control-character-in-input-stream") => {
                                ErrorKind::ControlCharacterInInputStream
                            }
                            Some("control-character-reference") => {
                                ErrorKind::ControlCharacterReference
                            }
                            Some("noncharacter-in-input-stream") => {
                                ErrorKind::NoncharacterInInputStream
                            }
                            Some("noncharacter-character-reference") => {
                                ErrorKind::NoncharacterCharacterReference
                            }
                            Some("unexpected-equals-sign-before-attribute-name") => {
                                ErrorKind::UnexpectedEqualsSignBeforeAttributeName
                            }
                            Some("unexpected-question-mark-instead-of-tag-name") => {
                                ErrorKind::UnexpectedQuestionMarkInsteadOfTagName
                            }
                            Some("unexpected-character-after-doctype-system-identifier") => {
                                ErrorKind::UnexpectedCharacterAfterDoctypeSystemIdentifier
                            }
                            Some("unexpected-null-character") => ErrorKind::UnexpectedNullCharacter,
                            Some("unexpected-solidus-in-tag") => ErrorKind::UnexpectedSolidusInTag,
                            Some("unexpected-character-in-attribute-name") => {
                                ErrorKind::UnexpectedCharacterInAttributeName
                            }
                            Some("unexpected-character-in-unquoted-attribute-value") => {
                                ErrorKind::UnexpectedCharacterInUnquotedAttributeValue
                            }
                            Some("duplicate-attribute") => ErrorKind::DuplicateAttribute,
                            Some("end-tag-with-attributes") => ErrorKind::EndTagWithAttributes,
                            Some("missing-whitespace-before-doctype-name") => {
                                ErrorKind::MissingWhitespaceBeforeDoctypeName
                            }
                            Some("missing-attribute-value") => ErrorKind::MissingAttributeValue,
                            Some("missing-doctype-public-identifier") => {
                                ErrorKind::MissingDoctypePublicIdentifier
                            }
                            Some("missing-end-tag-name") => ErrorKind::MissingEndTagName,
                            Some("missing-doctype-name") => ErrorKind::MissingDoctypeName,
                            Some("missing-doctype-system-identifier") => {
                                ErrorKind::MissingDoctypeSystemIdentifier
                            }
                            Some("missing-whitespace-after-doctype-system-keyword") => {
                                ErrorKind::MissingWhitespaceAfterDoctypeSystemKeyword
                            }
                            Some("missing-whitespace-after-doctype-public-keyword") => {
                                ErrorKind::MissingWhitespaceAfterDoctypePublicKeyword
                            }
                            Some("missing-quote-before-doctype-public-identifier") => {
                                ErrorKind::MissingQuoteBeforeDoctypePublicIdentifier
                            }
                            Some("missing-quote-before-doctype-system-identifier") => {
                                ErrorKind::MissingQuoteBeforeDoctypeSystemIdentifier
                            }
                            Some("incorrectly-closed-comment") => {
                                ErrorKind::IncorrectlyClosedComment
                            }
                            Some("invalid-character-sequence-after-doctype-name") => {
                                ErrorKind::InvalidCharacterSequenceAfterDoctypeName
                            }
                            Some(
                                "missing-whitespace-between-doctype-public-and-system-identifiers",
                            ) => {
                                ErrorKind::MissingWhitespaceBetweenDoctypePublicAndSystemIdentifiers
                            }
                            Some("missing-whitespace-between-attributes") => {
                                ErrorKind::MissingWhitespaceBetweenAttributes
                            }
                            Some("missing-semicolon-after-character-reference") => {
                                ErrorKind::MissingSemicolonAfterCharacterReference
                            }
                            Some("invalid-first-character-of-tag-name") => {
                                ErrorKind::InvalidFirstCharacterOfTagName
                            }
                            _ => {
                                unreachable!("unknown error {:?}", expected_code);
                            }
                        },
                        _ => {
                            unreachable!();
                        }
                    };

                    // TODO validate error positions
                    assert_eq!(
                        !actual_errors
                            .iter()
                            .filter(|&error| *error.kind() == expected_code)
                            .collect::<Vec<_>>()
                            .is_empty(),
                        true
                    );
                }
            } else {
                let errors = lexer.take_errors();

                assert_eq!(errors.len(), 0);
            }

            assert_eq!(actual, expected);
        }
    }
}
