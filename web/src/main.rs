use std::cell::Cell;
use std::rc::Rc;

use assembler::{AssembleError, Item, Program, Token};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Element, HtmlDocument, HtmlTextAreaElement, KeyboardEvent};

/// What the Tab key inserts in the editor.
const INDENT: &str = "    ";

/// How long to wait after the last keystroke before re-running the stages.
const DELAY_MS: i32 = 300;

/// Runs when the page loads: checks the code on every keystroke, and re-runs
/// every compilation stage once the user pauses typing.
fn main() {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let element = |id: &str| document.get_element_by_id(id).unwrap();
    let html_document: HtmlDocument = document.clone().dyn_into().unwrap();

    let editor: HtmlTextAreaElement = element("editor").dyn_into().unwrap();
    let backdrop = element("backdrop");
    let gutter = element("gutter");
    let status = element("status");
    let stages = [
        element("tokens"),
        element("labels"),
        element("parse"),
        element("encode"),
    ];

    // Redraws the error squiggles. Runs on every keystroke so they stay lined
    // up with the text.
    let check = {
        let (editor, backdrop, gutter) = (editor.clone(), backdrop.clone(), gutter.clone());
        move || {
            let source = editor.value();
            let errors = assembler::assemble(&source).err().unwrap_or_default();
            backdrop.set_inner_html(&render_backdrop(&source, &errors));
            gutter.set_inner_html(&render_gutter(&source, &errors));
            sync_scroll(&editor, &backdrop, &gutter);
        }
    };
    check();
    render_stages(&editor, &stages, &status);

    // Re-runs the stages; scheduled with a timeout after typing stops.
    let render = {
        let (editor, stages, status) = (editor.clone(), stages.clone(), status.clone());
        Closure::<dyn FnMut()>::new(move || render_stages(&editor, &stages, &status))
    };

    // On each keystroke: redraw squiggles, show the loading state, and restart
    // the timer so the stages only re-run once the user pauses.
    let timer: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    listen(&editor, "input", {
        let (stages, status) = (stages.clone(), status.clone());
        move || {
            check();

            status.set_class_name("status");
            status.set_inner_html("<span class=\"spinner\"></span>Compiling…");
            for stage in &stages {
                stage.set_class_name("stage-body loading");
            }

            if let Some(handle) = timer.take() {
                window.clear_timeout_with_handle(handle);
            }
            let handle = window
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    render.as_ref().unchecked_ref(),
                    DELAY_MS,
                )
                .unwrap();
            timer.set(Some(handle));
        }
    });

    // Tab inserts spaces instead of moving focus. The lexer doesn't accept tab
    // characters, so indent with spaces. `insertText` keeps undo working and
    // fires the input event, so errors are re-checked.
    let on_keydown = Closure::<dyn FnMut(KeyboardEvent)>::new(move |event: KeyboardEvent| {
        if event.key() == "Tab" && !event.shift_key() {
            event.prevent_default();
            let _ = html_document.exec_command_with_show_ui_and_value("insertText", false, INDENT);
        }
    });
    editor
        .add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())
        .unwrap();
    on_keydown.forget();

    // Keep the squiggles and line numbers lined up with the text.
    listen(&editor, "scroll", {
        let (editor, backdrop, gutter) = (editor.clone(), backdrop.clone(), gutter.clone());
        move || sync_scroll(&editor, &backdrop, &gutter)
    });
}

/// Runs every stage on the editor's code and shows the results.
fn render_stages(editor: &HtmlTextAreaElement, stages: &[Element; 4], status: &Element) {
    let program = assembler::analyze(&editor.value());

    stages[0].set_inner_html(&render_tokens(&program));
    stages[1].set_inner_html(&render_labels(&program));
    stages[2].set_inner_html(&render_parse(&program));
    stages[3].set_inner_html(&render_encode(&program));
    for stage in stages {
        stage.set_class_name("stage-body");
    }

    let (class, text) = describe_status(&program);
    status.set_class_name(class);
    status.set_text_content(Some(&text));
}

/// The header's summary of the last compile.
fn describe_status(program: &Program) -> (&'static str, String) {
    let errors = program.errors.len();
    if errors > 0 {
        let plural = if errors == 1 { "" } else { "s" };
        return ("status error", format!("{} error{}", errors, plural));
    }

    let words: usize = program.items.iter().map(|p| p.item.encode().len()).sum();
    let plural = if words == 1 { "" } else { "s" };
    ("status", format!("✓ Compiled · {} word{}", words, plural))
}

/// Calls `callback` whenever `event` fires on `target`.
fn listen(target: &Element, event: &str, callback: impl FnMut() + 'static) {
    let closure = Closure::<dyn FnMut()>::new(callback);
    target
        .add_event_listener_with_callback(event, closure.as_ref().unchecked_ref())
        .unwrap();

    // The closure has to live as long as the page, so hand it to JS.
    closure.forget();
}

fn sync_scroll(editor: &Element, backdrop: &Element, gutter: &Element) {
    backdrop.set_scroll_top(editor.scroll_top());
    backdrop.set_scroll_left(editor.scroll_left());
    gutter.set_scroll_top(editor.scroll_top());
}

/// Builds the layer drawn behind the (transparent) textarea: the same text,
/// invisible, with error lines highlighted, squiggled and annotated.
fn render_backdrop(source: &str, errors: &[AssembleError]) -> String {
    source
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let messages = messages_for(i + 1, errors);
            if messages.is_empty() {
                return format!("<div class=\"line\">{}</div>", escape(line));
            }

            // Only squiggle the code itself, not the surrounding whitespace.
            let code = line.trim();
            let start = line.find(code).unwrap_or(0);
            let (before, rest) = line.split_at(start);
            let after = &rest[code.len()..];

            format!(
                "<div class=\"line error\">{}<span class=\"squiggle\">{}</span>{}<span class=\"message\">{}</span></div>",
                escape(before),
                escape(code),
                escape(after),
                escape(&messages.join(" · ")),
            )
        })
        .collect()
}

/// Builds the line numbers, marking lines with errors.
fn render_gutter(source: &str, errors: &[AssembleError]) -> String {
    (1..=source.split('\n').count())
        .map(|line| {
            let class = if messages_for(line, errors).is_empty() {
                ""
            } else {
                " error"
            };
            format!("<div class=\"number{}\">{}</div>", class, line)
        })
        .collect()
}

fn messages_for(line: usize, errors: &[AssembleError]) -> Vec<&str> {
    errors
        .iter()
        .filter(|e| e.line == line)
        .map(|e| e.message.as_str())
        .collect()
}

/// Stage 1: each non-empty line's tokens, or why it failed to lex.
fn render_tokens(program: &Program) -> String {
    let rows: String = program
        .tokens
        .iter()
        .enumerate()
        .filter(|(_, tokens)| !matches!(tokens, Ok(tokens) if tokens.is_empty()))
        .map(|(i, tokens)| {
            let cell = match tokens {
                Ok(tokens) => tokens.iter().map(render_token).collect(),
                Err(message) => format!("<span class=\"error\">{}</span>", escape(message)),
            };
            format!("<tr><td class=\"ln\">{}</td><td>{}</td></tr>", i + 1, cell)
        })
        .collect();

    table_or_empty(&rows, "No tokens")
}

fn render_token(token: &Token) -> String {
    let (kind, lexeme) = describe(token);
    format!(
        "<span class=\"token\"><span class=\"kind\">{}</span>{}</span>",
        kind,
        escape(lexeme)
    )
}

/// A token's kind and the text it was made from.
fn describe(token: &Token) -> (&'static str, &str) {
    match token {
        Token::Dotid(lexeme) => ("DOTID", lexeme),
        Token::Label(lexeme) => ("LABEL", lexeme),
        Token::Id(lexeme) => ("ID", lexeme),
        Token::Hexint(lexeme) => ("HEXINT", lexeme),
        Token::Reg(lexeme) => ("REG", lexeme),
        Token::Zreg => ("ZREG", "xzr"),
        Token::Sp => ("SP", "sp"),
        Token::Int(lexeme) => ("INT", lexeme),
        Token::Comma => ("COMMA", ","),
        Token::Lbrack => ("LBRACK", "["),
        Token::Rbrack => ("RBRACK", "]"),
    }
}

/// Stage 2: the symbol table from pass 1.
fn render_labels(program: &Program) -> String {
    let rows: String = program
        .labels
        .iter()
        .map(|(label, index)| {
            format!(
                "<tr><td>{}</td><td class=\"muted\">word {}</td><td>0x{:04X}</td></tr>",
                escape(label),
                index,
                index * 4
            )
        })
        .collect();

    table_or_empty(&rows, "No labels")
}

/// Stage 3: what each line parsed into, with errors in place.
fn render_parse(program: &Program) -> String {
    let mut rows: Vec<(usize, String)> = program
        .items
        .iter()
        .map(|parsed| {
            let item = match &parsed.item {
                Item::Instruction(instruction) => format!("{:?}", instruction),
                Item::Data(value) => format!("Data(0x{:016X})", value),
            };
            let row = format!(
                "<tr><td class=\"ln\">{}</td><td>0x{:04X}</td><td>{}</td></tr>",
                parsed.line,
                parsed.index * 4,
                escape(&item)
            );
            (parsed.line, row)
        })
        .collect();

    rows.extend(program.errors.iter().map(|error| {
        let row = format!(
            "<tr><td class=\"ln\">{}</td><td></td><td class=\"error\">{}</td></tr>",
            error.line,
            escape(&error.message)
        );
        (error.line, row)
    }));
    rows.sort_by_key(|(line, _)| *line);

    let rows: String = rows.into_iter().map(|(_, row)| row).collect();
    table_or_empty(&rows, "Nothing to parse")
}

/// Stage 4: every output word, with its binary split into fields.
fn render_encode(program: &Program) -> String {
    if !program.errors.is_empty() {
        return String::from("<p class=\"empty\">Fix the errors to see the machine code</p>");
    }

    let rows: String = program
        .items
        .iter()
        .flat_map(|parsed| {
            let fields = parsed.item.fields();
            parsed
                .item
                .encode()
                .into_iter()
                .enumerate()
                .map(move |(i, word)| {
                    format!(
                        "<tr><td class=\"ln\">{}</td><td>0x{:04X}</td><td>{:08X}</td><td>{}</td></tr>",
                        parsed.line,
                        (parsed.index + i as i32) * 4,
                        word,
                        group_bits(word, fields)
                    )
                })
        })
        .collect();

    table_or_empty(&rows, "No machine code")
}

/// Splits a word's binary into one span per field, e.g. widths `[6, 26]`.
fn group_bits(word: u32, widths: &[u32]) -> String {
    let bits = format!("{:032b}", word);
    let mut start = 0;

    widths
        .iter()
        .map(|width| {
            let end = start + *width as usize;
            let field = format!("<span class=\"field\">{}</span>", &bits[start..end]);
            start = end;
            field
        })
        .collect()
}

fn table_or_empty(rows: &str, empty: &str) -> String {
    if rows.is_empty() {
        format!("<p class=\"empty\">{}</p>", empty)
    } else {
        format!("<table>{}</table>", rows)
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squiggles_only_the_code_on_error_lines() {
        let errors = vec![AssembleError {
            line: 2,
            message: String::from("bad <thing>"),
        }];
        assert_eq!(
            render_backdrop("ok\n  foo x1 ", &errors),
            "<div class=\"line\">ok</div>\
             <div class=\"line error\">  <span class=\"squiggle\">foo x1</span> \
             <span class=\"message\">bad &lt;thing&gt;</span></div>"
        );
    }

    #[test]
    fn marks_error_lines_in_gutter() {
        let errors = vec![AssembleError {
            line: 2,
            message: String::new(),
        }];
        assert_eq!(
            render_gutter("a\nb", &errors),
            "<div class=\"number\">1</div><div class=\"number error\">2</div>"
        );
    }

    #[test]
    fn groups_bits_by_field() {
        assert_eq!(
            group_bits(0x14000001, &[6, 26]),
            "<span class=\"field\">000101</span>\
             <span class=\"field\">00000000000000000000000001</span>"
        );
    }

    #[test]
    fn renders_tokens_with_kind_and_lexeme() {
        let program = assembler::analyze("\nb x1");
        assert_eq!(
            render_tokens(&program),
            "<table><tr><td class=\"ln\">2</td><td>\
             <span class=\"token\"><span class=\"kind\">ID</span>b</span>\
             <span class=\"token\"><span class=\"kind\">REG</span>x1</span>\
             </td></tr></table>"
        );
    }

    #[test]
    fn status_counts_words_or_errors() {
        assert_eq!(
            describe_status(&assembler::analyze("b 0\n.8byte 1")),
            ("status", String::from("✓ Compiled · 3 words"))
        );
        assert_eq!(
            describe_status(&assembler::analyze("foo\nbar")),
            ("status error", String::from("2 errors"))
        );
    }

    #[test]
    fn hides_machine_code_when_there_are_errors() {
        let program = assembler::analyze("foo");
        assert!(render_encode(&program).contains("Fix the errors"));
    }
}
